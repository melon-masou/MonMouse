use std::collections::HashMap;
use std::sync::mpsc::TryRecvError;

use crate::device_type::DeviceType;
use crate::device_type::WindowsRawinput;
use crate::errors::Result;
use crate::message::DeviceSetting;
use crate::message::DeviceStatus;
use crate::message::GenericDevice;
use crate::message::Message;
use crate::message::MouseControlReactor;
use crate::message::Positioning;
use crate::message::Settings;
use crate::mouse_control::DeviceController;
use crate::mouse_control::MonitorArea;
use crate::mouse_control::MonitorAreasList;
use crate::mouse_control::MousePos;
use crate::mouse_control::MouseRelocator;
use crate::utils::SimpleRatelimit;

use core::cell::OnceCell;
use log::{debug, error, trace};
use windows::Win32::UI::Input::RAWINPUTDEVICE;
use windows::Win32::UI::Input::RIDEV_PAGEONLY;
use windows::Win32::UI::WindowsAndMessaging::MsgWaitForMultipleObjects;
use windows::Win32::UI::WindowsAndMessaging::PeekMessageW;
use windows::Win32::UI::WindowsAndMessaging::PM_REMOVE;
use windows::Win32::UI::WindowsAndMessaging::QS_ALLINPUT;
use windows::Win32::UI::WindowsAndMessaging::WM_INPUT_DEVICE_CHANGE;
use windows::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, WPARAM},
    UI::{
        Input::{RAWINPUT, RAWINPUTDEVICELIST, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK},
        WindowsAndMessaging::{
            DispatchMessageW, TranslateMessage, HHOOK, MSG, MSLLHOOKSTRUCT, WM_INPUT, WM_QUIT,
        },
    },
};

use super::constants::*;
use super::wintypes::*;
use super::winwrap::*;

pub struct WinDevice {
    pub handle: HANDLE,
    pub device_type: DeviceType,
    pub id: Option<String>,
    pub rawinput: Option<RawinputInfo>,
    pub iface: Option<DeviceIfaceInfo>,
    pub parents: Vec<WString>,
    pub hid: Option<HidDeviceInfo>,
    pub ctrl: DeviceController,
}

impl std::fmt::Display for WinDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Dev({})", self.handle.0)?;

        let rawinput = match &self.rawinput {
            Some(v) => v,
            None => return writeln!(f, "Is a dummy device"),
        };

        writeln!(f, "iface: {}", rawinput.iface)?;
        writeln!(f, "dwType: {}", rawinput.typ())?;
        write!(f, "parents: [ ")?;
        for p in &self.parents {
            write!(f, "{} ", p)?;
        }
        writeln!(f, "]")?;

        match rawinput.typ() {
            RawDeviceType::MOUSE => {
                let mouse = rawinput.get_mouse();
                writeln!(f, "Is a Mouse")?;
                writeln!(f, "dwId: {}", mouse.dwId)?;
                writeln!(f, "dwNumberOfButtons: {}", mouse.dwNumberOfButtons)?;
                writeln!(f, "dwSampleRate: {}", mouse.dwSampleRate)?;
            }
            RawDeviceType::KEYBOARD => {
                let hid = rawinput.get_hid();
                writeln!(f, "Is HID")?;
                writeln!(f, "dwProductId: {}", hid.dwProductId)?;
                writeln!(f, "dwVendorId: {}", hid.dwVendorId)?;
                writeln!(f, "dwVersionNumber: {}", hid.dwVersionNumber)?;
                writeln!(f, "usUsagePage: {}", hid.usUsagePage)?;
                writeln!(f, "usUsage: {}", hid.usUsage)?;
            }
            _ => (),
        }
        match &self.iface {
            Some(infos) => {
                writeln!(f, "iface info::")?;
                writeln!(f, "instance_id: {}", infos.instance_id)?;
                writeln!(f, "name: {}", infos.name)?;
                writeln!(f, "service: {}", infos.service)?;
                writeln!(f, "class: {}", infos.class)?;
                writeln!(f, "manufacurer: {}", infos.manufacurer)?;
            }
            None => (),
        };
        match &self.hid {
            Some(infos) => {
                writeln!(f, "hid info::")?;
                writeln!(f, "serial_number: {}", infos.serial_number)?;
                writeln!(f, "product: {}", infos.product)?;
                writeln!(f, "manufacturer: {}", infos.manufacturer)?;
            }
            None => (),
        };
        Ok(())
    }
}

fn init_device_control(handle: HANDLE) -> DeviceController {
    let setting = DeviceSetting {
        locked_in_monitor: false,
        remember_pos: false,
    };
    DeviceController::new(handle.0 as u64, setting)
}

// A dummy device for WM_INPUT events which have null RAWINPUT.hDevice.
// Those may be from some precision touchpads. Official documents lack pages about this.
// Ref: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-rawinputheader
//      https://stackoverflow.com/questions/57552844/rawinputheader-hdevice-null-on-wm-input-for-laptop-trackpad
fn unassociated_events_capture_device() -> WinDevice {
    let handle = HANDLE(0);
    WinDevice {
        handle,
        id: Some(String::from("UnassociatedEventsCapture")),
        device_type: DeviceType::Dummy,
        rawinput: None,
        iface: None,
        parents: Vec::new(),
        hid: None,
        ctrl: init_device_control(handle),
    }
}

pub fn get_device_type(rawinput: &RawinputInfo) -> DeviceType {
    match rawinput.typ() {
        RawDeviceType::MOUSE => DeviceType::Mouse,
        RawDeviceType::KEYBOARD => DeviceType::Keyboard,
        RawDeviceType::HID => {
            let hid = rawinput.get_hid();
            DeviceType::from_hid_usage(hid.usUsagePage, hid.usUsage)
        }
        RawDeviceType::UNKNOWN => DeviceType::Unknown,
    }
}

fn collect_rawinput_infos(dev: &RAWINPUTDEVICELIST) -> Result<RawinputInfo> {
    let handlev = dev.hDevice.0;
    match device_collect_rawinput_infos(dev.hDevice) {
        Ok(v) => Ok(v),
        Err(e) => {
            error!("Get dev info failed({}): {}", handlev, e);
            Err(e)
        }
    }
}

fn collect_device_infos(
    handle: HANDLE,
    device_type: DeviceType,
    rawinput: RawinputInfo,
) -> Result<WinDevice> {
    let handlev = handle.0;
    let (iface, id) = match device_get_iface_infos(&rawinput.iface) {
        Ok(v) => {
            let id = v.instance_id.to_string();
            (Some(v), Some(id))
        }
        Err(e) => {
            error!(
                "Get iface info failed({}): {}. interface={}",
                handlev, e, rawinput.iface,
            );
            (None, None)
        }
    };
    let parents = match &iface {
        Some(i) => match device_get_parents(&i.instance_id, None) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "Get device parents failed({}): {}. interface={}",
                    handlev, e, rawinput.iface,
                );
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let hid = match (&iface, rawinput.typ()) {
        (Some(i), RawDeviceType::HID) => match device_get_hid_info(&i.instance_id, true) {
            Ok(v) => Some(v),
            Err(e) => {
                error!(
                    "Get hid info failed({}): {}. interface={}",
                    handlev, e, rawinput.iface
                );
                None
            }
        },
        _ => None,
    };
    let ctrl = init_device_control(handle);

    Ok(WinDevice {
        handle,
        id,
        device_type,
        rawinput: Some(rawinput),
        iface,
        parents,
        hid,
        ctrl,
    })
}

struct WinDeviceSet {
    devs: Vec<WinDevice>,
    indexs: HashMap<isize, usize>,
    active_id: Option<usize>,
}

impl WinDeviceSet {
    fn map_key(h: HANDLE) -> isize {
        h.0
    }

    pub fn new() -> WinDeviceSet {
        WinDeviceSet {
            devs: Vec::new(),
            indexs: HashMap::new(),
            active_id: None,
        }
    }

    pub fn active(&mut self) -> Option<&mut WinDevice> {
        if let Some(id) = self.active_id {
            self.devs.get_mut(id)
        } else {
            None
        }
    }

    pub fn get_and_update_active(&mut self, handle: HANDLE) -> Option<&mut WinDevice> {
        if let Some(id) = self.active_id {
            let active_handle = self.devs.get(id).unwrap().handle;
            if active_handle == handle {
                return self.active();
            }
        }
        self.active_id = self.indexs.get(&WinDeviceSet::map_key(handle)).copied();
        self.active()
    }

    pub fn rebuild(&mut self, new_devs: Vec<WinDevice>) {
        self.devs = new_devs;
        self.indexs = self
            .devs
            .iter()
            .enumerate()
            .map(|(i, d)| (WinDeviceSet::map_key(d.handle), i))
            .collect();
        self.active_id = None;
    }

    pub fn iter(&self) -> std::slice::Iter<'_, WinDevice> {
        self.devs.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, WinDevice> {
        self.devs.iter_mut()
    }
}

struct WinHook {
    mouse_ll_hook: Option<HHOOK>,
}

impl WinHook {
    fn new() -> Self {
        WinHook {
            mouse_ll_hook: None,
        }
    }
    fn register(&mut self) -> Result<()> {
        self.mouse_ll_hook = Some(set_windows_hook(HookWrap::mouse_ll::<WinHook>())?);
        Ok(())
    }
    fn unregister(&mut self) -> Result<()> {
        if let Some(h) = self.mouse_ll_hook {
            let _ = unset_windows_hook(h);
        }
        Ok(())
    }
}

impl MouseLowLevelHook for WinHook {
    fn on_mouse_ll(action: u32, e: &mut MSLLHOOKSTRUCT) -> bool {
        let processor = unsafe { G_PROCESSOR.get_mut().unwrap() };

        trace!(
            "mousell hook: action={}, pt=({},{})",
            action,
            e.pt.x,
            e.pt.y
        );

        let ctrl = processor.devices.active().map(|v| &mut v.ctrl);
        processor
            .relocator
            .on_pos_update(ctrl, MousePos::from(e.pt.x, e.pt.y));
        true
    }
}

struct WinDeviceProcessor {
    hwnd: HWND,
    devices: WinDeviceSet,
    headless: bool,

    raw_input_buf: WBuffer,
    tick_widen: TickWiden,
    relocator: MouseRelocator,
    cached_settings: Option<Settings>,
    to_update_devices: bool,

    rl_update_mon: SimpleRatelimit,
    rl_update_dev: SimpleRatelimit,
}
// Since Windows hook accept only a function pointer callback, not a closure.
// And it is hard to pass a WinDeviceProcessor instance as context to hook handler.
// To resolve this problem, we define the hook callback as static functions(defined in WinHook),
// the callback obtains the singleton instance WinDeviceProcessor as the context.
static mut G_PROCESSOR: OnceCell<WinDeviceProcessor> = OnceCell::new();

impl WinDeviceProcessor {
    fn new(headless: bool) -> Self {
        WinDeviceProcessor {
            // Window must be created within same thread where eventloop() is called. Value set at init().
            hwnd: HWND::default(),
            devices: WinDeviceSet::new(),
            headless,

            raw_input_buf: WBuffer::new(RAWINPUT_MSG_INIT_BUF_SIZE),
            tick_widen: TickWiden::new(),
            relocator: MouseRelocator::new(),
            cached_settings: None,
            to_update_devices: false,

            rl_update_mon: SimpleRatelimit::new(RATELIMIT_UPDATE_MONITOR_ONCE_MS),
            rl_update_dev: SimpleRatelimit::new(RATELIMIT_UPDATE_DEVICE_ONCE_MS),
        }
    }
}

impl WinDeviceProcessor {
    fn init_global_once(processor: WinDeviceProcessor) -> &'static mut WinDeviceProcessor {
        unsafe {
            if G_PROCESSOR.set(processor).is_err() {
                panic!("WinDeviceProcessor::init_global_once() called twice")
            }
            G_PROCESSOR.get_mut().unwrap()
        }
    }
    fn initialize(&mut self) -> Result<()> {
        self.hwnd = match create_dummy_window(None, None) {
            Ok(v) => v,
            Err(e) => {
                error!("Create dummy window failed: {}", e);
                return Err(e);
            }
        };
        match self.register_raw_devices() {
            Ok(_) => (),
            Err(e) => {
                error!("Register raw devices failed: {}", e);
                return Err(e);
            }
        };
        match self.try_update_monitors(true) {
            Ok(_) => (),
            Err(e) => {
                error!("Init monitors info failed: {}", e);
                return Err(e);
            }
        }
        match self.try_update_devices(true) {
            Ok(_) => (),
            Err(e) => {
                error!("Init devices info failed: {}", e);
                return Err(e);
            }
        }
        Ok(())
    }
    fn terminate(&mut self) -> Result<()> {
        Ok(())
    }
}

impl WinDeviceProcessor {
    fn filter_rawinput_devices(device_type: DeviceType) -> bool {
        device_type.is_pointer()
    }

    fn collect_all_raw_devices(&mut self) -> Result<Vec<WinDevice>> {
        let all_devs = match device_list_all() {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        Ok(all_devs
            .into_iter()
            .filter_map(|d| {
                let rawinput = match collect_rawinput_infos(&d) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to collect rawinput info({}): {}", d.hDevice.0, e);
                        return None;
                    }
                };
                let device_type = get_device_type(&rawinput);
                if !Self::filter_rawinput_devices(device_type) {
                    return None;
                }
                match collect_device_infos(d.hDevice, device_type, rawinput) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        error!("Failed to collect device info({}): {}", d.hDevice.0, e);
                        None
                    }
                }
            })
            .collect())
    }

    fn register_raw_devices(&mut self) -> Result<()> {
        let to_register: Vec<RAWINPUTDEVICE> = WindowsRawinput::REGISTER_USAGE_SET
            .iter()
            .map(|(page, usage)| {
                let mut flags = RIDEV_DEVNOTIFY | RIDEV_INPUTSINK;
                if usage == &WindowsRawinput::ALL {
                    flags |= RIDEV_PAGEONLY;
                }
                RAWINPUTDEVICE {
                    usUsage: *usage,
                    usUsagePage: *page,
                    dwFlags: flags,
                    hwndTarget: self.hwnd,
                }
            })
            .collect();
        register_rawinput_devices(&to_register)
    }

    fn monitor_area_from(mi: &MonitorInfo) -> MonitorArea {
        let actx = |x: i32| x * mi.scale as i32 / 100;
        MonitorArea {
            lefttop: MousePos::from(actx(mi.rect.left), actx(mi.rect.top)),
            rigtbtm: MousePos::from(actx(mi.rect.right), actx(mi.rect.bottom)),
            scale: mi.scale,
        }
    }
    fn phy_pos_from(p: &MousePos, scale: u32) -> (i32, i32) {
        (p.x * 100 / scale as i32, p.y * 100 / scale as i32)
    }

    fn try_update_devices(&mut self, must: bool) -> Result<()> {
        if !must && !self.rl_update_dev.allow(get_cur_tick()) {
            return Ok(());
        }

        let mut rawdevices = match self.collect_all_raw_devices() {
            Ok(v) => v,
            Err(e) => {
                error!("Collect all raw devices failed: {}", e);
                return Err(e);
            }
        };
        rawdevices.push(unassociated_events_capture_device());

        debug!("Updated rawdevices list: num={}", rawdevices.len());
        for d in rawdevices.iter() {
            debug!("Device: {}", d);
        }
        self.devices.rebuild(rawdevices);
        self.try_apply_settings(); // Apply cached settings again
        self.to_update_devices = false;
        Ok(())
    }

    fn try_update_monitors(&mut self, must: bool) -> Result<bool> {
        if !must && !self.rl_update_mon.allow(get_cur_tick()) {
            return Ok(false);
        }

        let mut mons = match get_all_monitors_info() {
            Ok(v) => v,
            Err(e) => {
                error!("Update monitors info failed: {}", e);
                return Err(e);
            }
        };
        if !self.headless {
            // If not running under headless mode, EnumDisplayMonitors() returns
            // right resolution, just clear the scale info.
            mons.iter_mut().for_each(|v| v.scale = 100);
        }
        let mon_areas = MonitorAreasList::from(
            mons.iter()
                .map(WinDeviceProcessor::monitor_area_from)
                .collect(),
        );
        debug!("Updated monitors: {}", mon_areas);
        self.relocator.update_monitors(mon_areas);
        Ok(true)
    }

    fn try_apply_settings(&mut self) {
        let settings = match &self.cached_settings {
            Some(v) => v,
            None => return,
        };

        let applyed: usize = settings.devices.iter().fold(0, |applyed, dev_setting| {
            let found_dev = self.devices.iter_mut().find(|v| {
                if let Some(id) = &v.id {
                    if id == &dev_setting.0 {
                        return true;
                    }
                }
                false
            });
            match found_dev {
                Some(d) => {
                    debug!("device {} apply settings: {}", dev_setting.0, dev_setting.1);
                    d.ctrl.update_settings(&dev_setting.1);
                    applyed + 1
                }
                None => applyed,
            }
        });

        if applyed < settings.devices.len() {
            debug!(
                "{} devices in cached_settings has not been applyed",
                settings.devices.len() - applyed
            );
        }
    }

    fn on_raw_input(&mut self, _wparam: WPARAM, lparam: LPARAM, tick: u32) {
        match get_rawinput_data(lparam_as_rawinput(lparam), &mut self.raw_input_buf) {
            Ok(_) => (),
            Err(e) => {
                error!("Get rawinput data failed: {}", e);
                return;
            }
        }

        let ri = self.raw_input_buf.get_ref::<RAWINPUT>();
        let wtick = self.tick_widen.widen(tick);

        trace!(
            "rawinput msg: tick={} msg {}",
            wtick,
            rawinput_to_string(ri)
        );

        match self.devices.get_and_update_active(ri.header.hDevice) {
            Some(dev) => {
                dev.ctrl
                    .update_positioning(match check_mouse_event_is_absolute(ri) {
                        Some(true) => Positioning::Absolute,
                        Some(false) => Positioning::Relative,
                        None => Positioning::Unknown,
                    });
                self.relocator.on_mouse_update(&mut dev.ctrl, wtick);
            }
            None => {
                self.to_update_devices = true;
            }
        };
        self.resolve_pending_updating_task()
    }

    fn resolve_pending_updating_task(&mut self) {
        if self.to_update_devices {
            let _ = self.try_update_devices(false);
        }

        if self.relocator.need_update_monitors() {
            if let Ok(true) = self.try_update_monitors(false) {
                self.relocator.done_update_monitors();
            }
        }

        if let Some((new_pos, scale)) = self.relocator.pop_relocate_pos() {
            let (x, y) = WinDeviceProcessor::phy_pos_from(&new_pos, scale);
            let _ = set_cursor_pos(x, y);
            debug!("Reset cursor to ({},{})", x, y);
        }
    }

    fn handle_message(&mut self, msg: &MSG) {
        match msg.message {
            WM_INPUT => self.on_raw_input(msg.wParam, msg.lParam, msg.time),
            WM_INPUT_DEVICE_CHANGE => {
                debug!("Trigger updating devices by WM_INPUT_DEVICE_CHANGE");
                self.to_update_devices = true;
            }
            _ => (),
        }
    }
}

pub struct WinEventLoop {
    hook: WinHook,
    processor: &'static mut WinDeviceProcessor,
}

impl Default for WinEventLoop {
    fn default() -> Self {
        Self::new(false)
    }
}

impl WinEventLoop {
    pub fn new(headless: bool) -> Self {
        let hook = WinHook::new();
        let processor = WinDeviceProcessor::init_global_once(WinDeviceProcessor::new(headless));
        WinEventLoop { hook, processor }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.processor.initialize()?;
        self.hook.register()?;
        Ok(())
    }

    pub fn terminate(&mut self) -> Result<()> {
        self.hook.unregister()?;
        self.processor.terminate()?;
        Ok(())
    }

    #[inline]
    pub fn poll(&mut self, mut max_events: u32, timeout_ms: u32) -> Result<bool> {
        let mut msg = MSG::default();

        unsafe {
            MsgWaitForMultipleObjects(None, false, timeout_ms, QS_ALLINPUT);
            while max_events > 0
                && PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool()
            {
                if msg.message == WM_QUIT {
                    return Ok(false);
                }
                self.processor.handle_message(&msg);
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
                max_events -= 1;
            }
        }

        // Also try to update resources if need, though no external messages come
        self.processor.resolve_pending_updating_task();

        Ok(true)
    }

    pub fn run(&mut self) -> Result<()> {
        self.initialize()?;
        loop {
            if !self.poll(
                WIN_EVENTLOOP_POLL_MAX_MESSAGES,
                WIN_EVENTLOOP_POLL_WAIT_TIMEOUT_MS,
            )? {
                break;
            }
        }
        self.terminate()?;
        Ok(())
    }
}

impl WinEventLoop {
    pub fn poll_message(&mut self, mouse_control_reactor: &MouseControlReactor) {
        loop {
            let msg = match mouse_control_reactor.mouse_control_rx.try_recv() {
                Ok(msg) => msg,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => return,
            };

            // Is it possible to reuse the msg?
            match msg {
                Message::ScanDevices(_, _) => {
                    let ret = match self.processor.try_update_devices(true) {
                        Ok(_) => Ok(self
                            .processor
                            .devices
                            .iter()
                            .filter(|&v| Self::is_valid_win_device(v))
                            .map(Self::win_device_to_generic)
                            .collect()),
                        Err(e) => Err(e),
                    };
                    mouse_control_reactor.return_msg(Message::ScanDevices((), ret));
                }
                Message::InspectDevicesStatus(_, _) => {
                    let tick = get_cur_tick();
                    let ret = self
                        .processor
                        .devices
                        .iter()
                        .filter(|&v| Self::is_valid_win_device(v))
                        .map(|d| {
                            (
                                d.id.as_ref().unwrap().clone(),
                                Self::build_device_status(d, tick),
                            )
                        })
                        .collect();
                    mouse_control_reactor.return_msg(Message::InspectDevicesStatus((), Ok(ret)));
                }
                Message::ApplyDevicesSetting(settings, _) => {
                    self.processor.cached_settings = Some(settings.unwrap());
                    self.processor.try_apply_settings();
                    mouse_control_reactor.return_msg(Message::ApplyDevicesSetting(None, Ok(())));
                }
                _ => panic!("recv unexpected ui msg: {}", msg),
            }
        }
    }

    pub fn is_valid_win_device(d: &WinDevice) -> bool {
        d.id.is_some()
    }

    pub fn win_device_to_generic(d: &WinDevice) -> GenericDevice {
        GenericDevice {
            id: d.id.as_ref().unwrap().to_string(),
            device_type: d.device_type,
            product_name: Self::build_product_name(d).trim().into(),
            platform_specific_infos: Self::build_platform_specific_infos(d),
        }
    }

    pub fn build_device_status(d: &WinDevice, cur_tick: u64) -> DeviceStatus {
        if let Some((last_tick, _, positioning)) = d.ctrl.get_last_pos() {
            if last_tick + MOUSE_EVENT_ACTIVE_LAST_FOR_MS > cur_tick {
                DeviceStatus::Active(positioning)
            } else {
                DeviceStatus::Idle
            }
        } else {
            DeviceStatus::Idle
        }
    }

    pub fn build_product_name(d: &WinDevice) -> String {
        if let Some(hid) = &d.hid {
            let mut name = String::new();
            if let WStringOption::Some(s) = &hid.manufacturer {
                name.push_str(s.to_string().as_str());
                name.push(' ');
            }
            if let WStringOption::Some(s) = &hid.product {
                name.push_str(s.to_string().as_str());
                name.push(' ');
            }
            if !name.is_empty() {
                return name;
            }
        };
        if let Some(iface) = &d.iface {
            let mut name = if let WStringOption::Some(s) = &iface.manufacurer {
                let mut s = s.to_string();
                s.push(' ');
                s
            } else {
                String::new()
            };
            name.push_str(iface.name.to_string().as_str());
            return name;
        }
        d.id.as_ref().unwrap().clone()
    }

    pub fn build_platform_specific_infos(d: &WinDevice) -> Vec<(String, String)> {
        let tag = |s: &str| s.to_owned();

        let rawinput = match &d.rawinput {
            Some(v) => v,
            None => return Vec::new(),
        };

        let mut vs = vec![
            (tag("interface"), rawinput.iface.to_string()),
            (tag("dwType"), rawinput.rid_info.dwType.0.to_string()),
        ];
        if let Some(hm) = &d.hid {
            if let WStringOption::Some(s) = &hm.manufacturer {
                vs.push((tag("hidManufacurer"), s.to_string()));
            }
            if let WStringOption::Some(s) = &hm.product {
                vs.push((tag("hidProduct"), s.to_string()));
            }
            if let WStringOption::Some(s) = &hm.serial_number {
                vs.push((tag("hidSerialNumber"), s.to_string()));
            }
        }

        if let Some(im) = &d.iface {
            if let WStringOption::Some(s) = &im.manufacurer {
                vs.push((tag("manufacurer"), s.to_string()));
            }
            if let WStringOption::Some(s) = &im.name {
                vs.push((tag("name"), s.to_string()));
            }
            if let WStringOption::Some(s) = &im.service {
                vs.push((tag("service"), s.to_string()));
            }
            if let WStringOption::Some(s) = &im.class {
                vs.push((tag("class"), s.to_string()));
            }
        }

        match rawinput.typ() {
            RawDeviceType::MOUSE => {
                let m = &rawinput.get_mouse();
                vs.push((tag("dwId"), m.dwId.to_string()));
                vs.push((tag("dwNumberOfButtons"), m.dwNumberOfButtons.to_string()));
                vs.push((tag("dwSampleRate"), m.dwSampleRate.to_string()));
            }
            RawDeviceType::KEYBOARD => (),
            RawDeviceType::HID => {
                let m = &rawinput.get_hid();
                vs.push((tag("dwProductId"), m.dwProductId.to_string()));
                vs.push((tag("dwVendorId"), m.dwVendorId.to_string()));
                vs.push((tag("dwVersionNumber"), m.dwVersionNumber.to_string()));
                vs.push((tag("usUsagePage"), format!("0x{:X}", m.usUsagePage)));
                vs.push((tag("usUsage"), format!("0x{:X}", m.usUsage)));
            }
            RawDeviceType::UNKNOWN => (),
        }

        vs
    }
}
