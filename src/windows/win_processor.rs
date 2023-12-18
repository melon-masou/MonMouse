use std::collections::HashMap;

use crate::errors::Result;

use crate::message::DeviceType as GenericDeviceType;
use crate::message::GenericDevice;
use crate::message::Message;
use crate::message::MouseControlReactor;
use crate::mouse_control::DeviceController;
use crate::mouse_control::DeviceCtrlSetting;
use crate::mouse_control::MonitorArea;
use crate::mouse_control::MonitorAreasList;
use crate::mouse_control::MousePos;
use crate::mouse_control::MouseRelocator;
use crate::utils::ArrayVec;
use crate::utils::SimpleRatelimit;

use core::cell::OnceCell;
use log::{debug, error, trace};
use windows::Win32::Devices::HumanInterfaceDevice::HID_USAGE_GENERIC_MOUSE;
use windows::Win32::Devices::HumanInterfaceDevice::HID_USAGE_GENERIC_POINTER;
use windows::Win32::Devices::HumanInterfaceDevice::HID_USAGE_PAGE_DIGITIZER;
use windows::Win32::{
    Devices::HumanInterfaceDevice::HID_USAGE_PAGE_GENERIC,
    Foundation::{HANDLE, HWND, LPARAM, WPARAM},
    UI::{
        Input::{RAWINPUT, RAWINPUTDEVICELIST, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK},
        WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, TranslateMessage, HHOOK, MSG, MSLLHOOKSTRUCT, WM_INPUT,
            WM_QUIT,
        },
    },
};

use super::constants::*;
use super::wintypes::*;
use super::winwrap::*;

pub struct WinDevice {
    pub handle: HANDLE,
    pub rawinput: RawinputInfo,
    pub iface: Option<DeviceIfaceInfo>,
    pub parents: Vec<WString>,
    pub hid: Option<HidDeviceInfo>,
    pub ctrl: DeviceController,
}

impl std::fmt::Display for WinDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Dev({})", self.handle.0)?;

        writeln!(f, "iface: {}", self.rawinput.iface)?;
        writeln!(f, "dwType: {}", self.rawinput.typ())?;
        write!(f, "parents: [ ")?;
        for p in &self.parents {
            write!(f, "{} ", p)?;
        }
        writeln!(f, "]")?;

        match self.rawinput.typ() {
            DeviceType::MOUSE => {
                let mouse = self.rawinput.get_mouse();
                writeln!(f, "Is a Mouse")?;
                writeln!(f, "dwId: {}", mouse.dwId)?;
                writeln!(f, "dwNumberOfButtons: {}", mouse.dwNumberOfButtons)?;
                writeln!(f, "dwSampleRate: {}", mouse.dwSampleRate)?;
            }
            DeviceType::KEYBOARD => {
                let hid = self.rawinput.get_hid();
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

fn init_device_control(handle: HANDLE, rawinput: &RawinputInfo) -> DeviceController {
    // TODO: set setting values
    let setting = DeviceCtrlSetting {
        restrict_in_monitor: false,
        remember_pos: rawinput.typ() == DeviceType::MOUSE,
    };

    DeviceController::new(handle.0 as u64, setting)
}

fn collect_device_infos(dev: &RAWINPUTDEVICELIST) -> Result<WinDevice> {
    let handlev = dev.hDevice.0;
    let rawinput = match device_collect_rawinput_infos(dev.hDevice) {
        Ok(v) => v,
        Err(e) => {
            error!("Get dev info failed({}): {}", handlev, e);
            return Err(e);
        }
    };

    let iface = match device_get_iface_infos(&rawinput.iface) {
        Ok(v) => Some(v),
        Err(e) => {
            error!(
                "Get iface info failed({}): {}. interface={}",
                handlev, e, rawinput.iface,
            );
            None
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
        (Some(i), DeviceType::HID) => match device_get_hid_info(&i.instance_id, true) {
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
    let ctrl = init_device_control(dev.hDevice, &rawinput);

    Ok(WinDevice {
        handle: dev.hDevice,
        rawinput,
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

    raw_input_buf: WBuffer,
    tick_widen: TickWiden,
    relocator: MouseRelocator,

    rl_update_mon: SimpleRatelimit,
    rl_update_dev: SimpleRatelimit,
}
// Since Windows hook accept only a function pointer callback, not a closure.
// And it is hard to pass a WinDeviceProcessor instance as context to hook handler.
// To resolve this problem, we define the hook callback as static functions(defined in WinHook),
// the callback obtains the singleton instance WinDeviceProcessor as the context.
static mut G_PROCESSOR: OnceCell<WinDeviceProcessor> = OnceCell::new();

impl WinDeviceProcessor {
    fn new() -> Self {
        WinDeviceProcessor {
            // Window must be created within same thread where eventloop() is called. Value set at init().
            hwnd: HWND::default(),
            devices: WinDeviceSet::new(),

            raw_input_buf: WBuffer::new(RAWINPUT_MSG_INIT_BUF_SIZE),
            tick_widen: TickWiden::new(),
            relocator: MouseRelocator::new(),

            rl_update_mon: SimpleRatelimit::new(RATELIMIT_UPDATE_MONITOR_ONCE_MS),
            rl_update_dev: SimpleRatelimit::new(RATELIMIT_UPDATE_DEVICE_ONCE_MS),
        }
    }
}

impl WinDeviceProcessor {
    fn init_global_once() -> &'static mut WinDeviceProcessor {
        unsafe {
            if G_PROCESSOR.set(WinDeviceProcessor::new()).is_err() {
                panic!("WinDeviceProcessor::init_global_once() called twice")
            }
            G_PROCESSOR.get_mut().unwrap()
        }
    }
    fn initialize(&mut self) -> Result<()> {
        self.hwnd = create_dummy_window(None, None)?;
        self.try_update_monitors(true)?;
        self.try_update_devices(true)?;

        Ok(())
    }
    fn terminate(&mut self) -> Result<()> {
        Ok(())
    }
}

impl WinDeviceProcessor {
    fn collect_all_raw_devices(&mut self) -> Result<Vec<WinDevice>> {
        let all_devs = match device_list_all() {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        let r: Result<Vec<WinDevice>> = all_devs
            .iter()
            .filter(|d| match DeviceType::from_rid(d.dwType) {
                DeviceType::MOUSE | DeviceType::HID => true,
                DeviceType::KEYBOARD | DeviceType::UNKNOWN => false,
            })
            .map(collect_device_infos)
            .filter(|r| r.is_ok()) // ignore certain device error
            .collect();
        r
    }

    fn register_raw_devices(&mut self) -> Result<()> {
        let devs = vec![
            rawinput_reg(
                self.hwnd,
                HID_USAGE_GENERIC_POINTER,
                HID_USAGE_PAGE_DIGITIZER,
                RIDEV_DEVNOTIFY | RIDEV_INPUTSINK,
            ),
            rawinput_reg(
                self.hwnd,
                HID_USAGE_GENERIC_MOUSE,
                HID_USAGE_PAGE_GENERIC,
                RIDEV_DEVNOTIFY | RIDEV_INPUTSINK,
            ),
        ];
        register_rawinput_devices(&devs)
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

    fn try_update_devices(&mut self, must: bool) -> Result<bool> {
        if !must && !self.rl_update_dev.allow(get_cur_tick()) {
            return Ok(false);
        }

        let rawdevices = match self.collect_all_raw_devices() {
            Ok(v) => v,
            Err(e) => {
                error!("Collect all raw devices failed: {}", e);
                return Err(e);
            }
        };

        debug!("Updated rawdevices list: num={}", rawdevices.len());
        for d in rawdevices.iter() {
            debug!("Device: {}", d);
        }
        self.devices.rebuild(rawdevices);

        match self.register_raw_devices() {
            Ok(_) => (),
            Err(e) => {
                error!("Register raw devices failed: {}", e)
            }
        };

        Ok(true)
    }

    fn try_update_monitors(&mut self, must: bool) -> Result<bool> {
        if !must && !self.rl_update_mon.allow(get_cur_tick()) {
            return Ok(false);
        }

        let mons = match get_all_monitors_info() {
            Ok(v) => v,
            Err(e) => {
                error!("Update monitors info failed: {}", e);
                return Err(e);
            }
        };
        let mon_areas = MonitorAreasList::from(
            mons.iter()
                .map(WinDeviceProcessor::monitor_area_from)
                .collect(),
        );
        trace!("Updated monitors: {}", mon_areas);
        self.relocator.update_monitors(mon_areas);
        Ok(true)
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

        // TODO: hDevice can be zero if an input is received from a precision touchpad

        let dev = match self.devices.get_and_update_active(ri.header.hDevice) {
            Some(v) => v,
            None => return,
        };
        self.relocator.on_mouse_update(&mut dev.ctrl, wtick);
        self.resolve_relocator()
    }

    fn resolve_relocator(&mut self) {
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
        Self::new()
    }
}

impl WinEventLoop {
    pub fn new() -> Self {
        let hook = WinHook::new();
        let processor = WinDeviceProcessor::init_global_once();
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
    pub fn poll(&mut self) -> Result<bool> {
        let mut msg = MSG::default();
        if unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.as_bool() {
            if msg.message == WM_QUIT {
                return Ok(false);
            }
            self.processor.handle_message(&msg);
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(true)
    }

    pub fn run(&mut self) -> Result<()> {
        self.initialize()?;
        loop {
            if self.poll()? {
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
            let msg = match mouse_control_reactor.recv_msg() {
                Some(msg) => msg,
                None => return,
            };
            // Is it possible to reuse the msg?
            match msg {
                Message::InspectDevices(_, _) => {
                    let ret =
                        self.processor
                            .collect_all_raw_devices()
                            .map(|v| -> Vec<GenericDevice> {
                                v.iter()
                                    .filter(|&v| Self::win_device_filter(v))
                                    .map(Self::win_device_to_generic)
                                    .collect()
                            });
                    mouse_control_reactor.return_msg(Message::InspectDevices((), ret));
                }
                Message::ApplyDevicesSetting() => todo!(),
                _ => panic!("recv unexpected ui msg: {}", msg),
            }
        }
    }

    pub fn win_device_filter(d: &WinDevice) -> bool {
        d.iface.is_some()
            && match d.rawinput.typ() {
                DeviceType::MOUSE => true,
                DeviceType::KEYBOARD => false,
                DeviceType::HID => true,
                DeviceType::UNKNOWN => false,
            }
    }

    pub fn win_device_to_generic(d: &WinDevice) -> GenericDevice {
        GenericDevice {
            id: d.iface.as_ref().unwrap().instance_id.to_string(),
            device_type: Self::get_device_type(d),
            product_name: Self::build_product_name(d).trim().into(),
            platform_specific_infos: Vec::new(),
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
            if let WStringOption::Some(s) = &hid.serial_number {
                name.push_str(s.to_string().as_str());
                name.push(' ');
            }
            if !name.is_empty() {
                return name;
            }
        };

        let iface = d.iface.as_ref().unwrap();
        let mut name = if let WStringOption::Some(s) = &iface.manufacurer {
            let mut s = s.to_string();
            s.push(' ');
            s
        } else {
            String::new()
        };
        name.push_str(iface.name.to_string().as_str());
        name
    }

    pub fn get_device_type(d: &WinDevice) -> GenericDeviceType {
        match d.rawinput.typ() {
            DeviceType::MOUSE => GenericDeviceType::Mouse,
            DeviceType::KEYBOARD => GenericDeviceType::Unknown,
            DeviceType::HID => GenericDeviceType::HIDUnknown,
            DeviceType::UNKNOWN => GenericDeviceType::Unknown,
        }
    }
}
