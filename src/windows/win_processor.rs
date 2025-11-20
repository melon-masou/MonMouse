use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use crate::device_type::DeviceType;
use crate::device_type::WindowsRawinput;
use crate::errors::Error;
use crate::errors::Result;
use crate::keyboard::key_windows::shortcut_str_to_win;
use crate::message::DeviceStatus;
use crate::message::GenericDevice;
use crate::message::Message;
use crate::message::MessageSender;
use crate::message::MouseControlReactor;
use crate::message::Positioning;
use crate::message::ShortcutID;
use crate::message::SysMouseEvent;
use crate::mouse_control::DeviceController;
use crate::mouse_control::MonitorArea;
use crate::mouse_control::MonitorAreasList;
use crate::mouse_control::MousePos;
use crate::mouse_control::MouseRelocator;
use crate::mouse_control::RelocatePos;
use crate::setting::DeviceSetting;
use crate::setting::ProcessorSettings;
use crate::setting::Settings;
use crate::utils::SimpleRatelimit;

use log::{debug, error, trace, warn};
use windows::Win32::UI::Input::RAWINPUTDEVICE;
use windows::Win32::UI::Input::RIDEV_PAGEONLY;
use windows::Win32::UI::WindowsAndMessaging::MsgWaitForMultipleObjects;
use windows::Win32::UI::WindowsAndMessaging::PeekMessageW;
use windows::Win32::UI::WindowsAndMessaging::PM_REMOVE;
use windows::Win32::UI::WindowsAndMessaging::QS_ALLINPUT;
use windows::Win32::UI::WindowsAndMessaging::WM_DISPLAYCHANGE;
use windows::Win32::UI::WindowsAndMessaging::WM_DPICHANGED;
use windows::Win32::UI::WindowsAndMessaging::WM_HOTKEY;
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
        if let Some(infos) = &self.iface {
            writeln!(f, "iface info::")?;
            writeln!(f, "instance_id: {}", infos.instance_id)?;
            writeln!(f, "name: {}", infos.name)?;
            writeln!(f, "service: {}", infos.service)?;
            writeln!(f, "class: {}", infos.class)?;
            writeln!(f, "manufacurer: {}", infos.manufacurer)?;
        };
        if let Some(infos) = &self.hid {
            writeln!(f, "hid info::")?;
            writeln!(f, "serial_number: {}", infos.serial_number)?;
            writeln!(f, "product: {}", infos.product)?;
            writeln!(f, "manufacturer: {}", infos.manufacturer)?;
        };
        Ok(())
    }
}

fn init_device_control(handle: HANDLE) -> DeviceController {
    let setting = DeviceSetting {
        locked_in_monitor: false,
        switch: false,
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

    pub fn active_id(&mut self) -> Option<&String> {
        self.active().and_then(|d| d.id.as_ref())
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

    pub fn update_one<R>(&mut self, id: &str, f: impl FnOnce(&mut WinDevice) -> R) -> Option<R> {
        self.iter_mut()
            .find(|v| {
                if let Some(found_id) = &v.id {
                    if found_id == id {
                        return true;
                    }
                }
                false
            })
            .map(f)
    }
    pub fn update_one_device_settings(&mut self, id: &str, s: &DeviceSetting) -> bool {
        self.update_one(id, |d| d.ctrl.update_settings(s)).is_some()
    }
}

struct WinHook {
    mouse_ll_hook: Option<HHOOK>,
}

impl WinHook {
    fn new() -> Self {
        let _ = G_HOOK_EV_SENDER.get().unwrap();
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
    fn set_ev_sender(sender: MessageSender) {
        G_HOOK_EV_SENDER.set(sender).unwrap();
    }
}

static G_HOOK_EV_SENDER: OnceLock<MessageSender> = OnceLock::new();
impl MouseLowLevelHook for WinHook {
    fn on_mouse_ll(action: u32, e: &mut MSLLHOOKSTRUCT) -> bool {
        let hook_ev_sender = G_HOOK_EV_SENDER.get().unwrap();

        trace!(
            "mousell hook: action={}, pt=({},{})",
            action,
            e.pt.x,
            e.pt.y
        );

        hook_ev_sender.send(Message::SysMouseEvent(SysMouseEvent {
            pos_x: e.pt.x,
            pos_y: e.pt.y,
        }));
        true
    }
}

struct WinDeviceProcessor {
    hwnd: HWND,
    devices: WinDeviceSet,

    raw_input_buf: WBuffer,
    tick_widen: TickWiden,
    relocator: MouseRelocator,
    settings: ProcessorSettings,
    to_update_devices: bool,
    to_update_monitors: bool,

    rl_update_mon: SimpleRatelimit,
    rl_update_dev: SimpleRatelimit,
}

impl WinDeviceProcessor {
    fn new() -> Self {
        WinDeviceProcessor {
            // Window must be created within same thread where eventloop() is called. Value set at init().
            hwnd: HWND::default(),
            devices: WinDeviceSet::new(),

            raw_input_buf: WBuffer::new(RAWINPUT_MSG_INIT_BUF_SIZE),
            tick_widen: TickWiden::new(),
            relocator: MouseRelocator::new(),
            settings: ProcessorSettings::default(),
            to_update_devices: false,
            to_update_monitors: false,

            rl_update_mon: SimpleRatelimit::new(
                Duration::from_millis(RATELIMIT_UPDATE_MONITOR_ONCE_MS),
                None,
            ),
            rl_update_dev: SimpleRatelimit::new(
                Duration::from_millis(RATELIMIT_UPDATE_DEVICE_ONCE_MS),
                None,
            ),
        }
    }
}

impl WinDeviceProcessor {
    fn initialize(&mut self) -> Result<()> {
        match self.register_raw_devices() {
            Ok(_) => (),
            Err(e) => {
                error!("Register raw devices failed: {}", e);
                return Err(e);
            }
        };
        // No need call self.try_update_devices(). Register raw devices will trigger RAW_DEVICE_CHANGE
        match self.try_update_monitors(true) {
            Ok(_) => (),
            Err(e) => {
                error!("Init monitors info failed: {}", e);
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
        let all_devs = device_list_all()?;
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

    fn filter_monitor(mon: &MonitorInfo) -> bool {
        mon.rect.left < mon.rect.right && mon.rect.top < mon.rect.bottom
    }
    fn monitor_area_from(mi: &MonitorInfo) -> MonitorArea {
        MonitorArea::new(
            MousePos::from(mi.rect.left, mi.rect.top),
            MousePos::from(mi.rect.right, mi.rect.bottom),
        )
    }

    fn try_update_devices(&mut self, must: bool) -> Result<()> {
        if !must && !self.rl_update_dev.allow(None).0 {
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
        self.apply_processor_settings(None); // Apply settings again
        self.to_update_devices = false;
        Ok(())
    }

    fn try_update_monitors(&mut self, must: bool) -> Result<()> {
        if !must && !self.rl_update_mon.allow(None).0 {
            return Ok(());
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
                .filter(|v| Self::filter_monitor(v))
                .map(WinDeviceProcessor::monitor_area_from)
                .collect(),
        );
        debug!("Updated monitors: {}", mon_areas);
        self.relocator.update_monitors(mon_areas);
        self.devices.iter_mut().for_each(|v| {
            v.ctrl.reset();
        });
        self.to_update_monitors = false;
        Ok(())
    }

    fn cur_mouse_lock_toogle(&mut self) {
        let device = self.devices.active();
        let Some(device) = device else {
            return;
        };
        let Some(id) = &device.id else {
            return;
        };
        let content = self.settings.ensure_mut_device(id, |d| {
            d.locked_in_monitor = !d.locked_in_monitor;
            *d
        });
        device.ctrl.update_settings(&content);
    }

    fn apply_processor_settings(&mut self, new_settings: Option<ProcessorSettings>) {
        if let Some(new) = new_settings {
            self.settings = new;
        }
        let settings = &self.settings;

        let applied: usize = settings.devices.iter().fold(0, |applied, item| {
            let found = self
                .devices
                .update_one_device_settings(&item.id, &item.content);
            if found {
                applied + 1
            } else {
                applied
            }
        });

        debug!(
            "{} in {} devices setting has not been applied",
            applied,
            settings.devices.len()
        );
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
        let positioning = match check_mouse_event_is_absolute(ri) {
            Some(true) => Positioning::Absolute,
            Some(false) => Positioning::Relative,
            None => Positioning::Unknown,
        };

        trace!(
            "rawinput msg: tick={} msg {}",
            wtick,
            rawinput_to_string(ri)
        );

        // Try merging unassociated event
        if ri.header.hDevice == HANDLE(0) {
            // If configured
            if self.settings.merge_unassociated_events_ms >= 0 {
                let merge_within = self.settings.merge_unassociated_events_ms as u64;
                // If active device exists
                if let Some(active_dev) = self.devices.active() {
                    if let Some((active_tick, _, _)) = active_dev.ctrl.get_last_pos() {
                        // If within time range
                        if active_tick + merge_within >= wtick {
                            // Eat the unassociated event
                            active_dev.ctrl.update_positioning(positioning);
                            self.relocator.on_mouse_update(&mut active_dev.ctrl, wtick);
                            return;
                        }
                    }
                }
            }
        }

        match self.devices.get_and_update_active(ri.header.hDevice) {
            Some(dev) => {
                dev.ctrl.update_positioning(positioning);
                self.relocator.on_mouse_update(&mut dev.ctrl, wtick);
            }
            None => {
                self.to_update_devices = true;
            }
        };
        self.resolve_pending_updating_task();
        self.resolve_relocation();
    }

    fn on_mouse_event(&mut self, ev: &SysMouseEvent) {
        let ctrl = self.devices.active().map(|v| &mut v.ctrl);
        self.relocator
            .on_pos_update(ctrl, MousePos::from(ev.pos_x, ev.pos_y));
    }

    fn resolve_pending_updating_task(&mut self) {
        if self.relocator.pop_need_update_monitors() {
            self.to_update_monitors = true;
        }

        if self.to_update_devices {
            let _ = self.try_update_devices(false);
        }
        if self.to_update_monitors {
            let _ = self.try_update_monitors(false);
        }
    }

    fn resolve_relocation(&mut self) {
        if let Some(RelocatePos(new_pos)) = self.relocator.pop_relocate_pos() {
            let MousePos { x, y } = new_pos;
            let _ = set_cursor_pos(x, y);
            debug!("Reset cursor to ({},{})", x, y);
        }
    }
}

pub struct WinEventLoop {
    hook: WinHook,
    processor: WinDeviceProcessor,
    headless: bool,
    hotkey_mgr: HotKeyManager<ShortcutID>,
    mouse_control_reactor: MouseControlReactor,
}

impl SubclassHandler for WinEventLoop {
    fn subclass_callback(&mut self, umsg: u32, _wp: WPARAM, _lp: LPARAM, _class: usize) -> bool {
        match umsg {
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                debug!("Trigger updating monitors by WM {}", umsg);
                self.processor.to_update_monitors = true;
            }
            _ => (),
        }
        true
    }
}

impl WinEventLoop {
    fn apply_one_shortcut(
        mgr: &mut HotKeyManager<ShortcutID>,
        hwnd: HWND,
        shortcut_str: &str,
        id: ShortcutID,
    ) -> Result<()> {
        if shortcut_str.is_empty() {
            let _ = mgr.unregister(hwnd, id as i32);
            return Ok(());
        }
        let _ = mgr.unregister(hwnd, id as i32);
        match shortcut_str_to_win(shortcut_str) {
            Some((modifier, key)) => {
                match mgr.register(hwnd, id as i32, modifier, key, false, id) {
                    Err(Error::ShortcutConflict(_)) => {
                        Err(Error::ShortcutConflict(shortcut_str.into()))
                    }
                    res => res,
                }
            }
            None => Err(Error::InvalidShortcut(shortcut_str.to_owned())),
        }
    }

    fn register_shortcuts(&mut self) -> Result<()> {
        let shortcuts = &self.processor.settings.shortcuts;
        let mut last_error: Result<()> = Ok(());

        if let Err(e) = Self::apply_one_shortcut(
            &mut self.hotkey_mgr,
            self.processor.hwnd,
            &shortcuts.cur_mouse_lock,
            ShortcutID::CurMouseLock,
        ) {
            error!("register shortcut cur_mouse_lock error: {}", e);
            last_error = Err(e);
        }

        if let Err(e) = Self::apply_one_shortcut(
            &mut self.hotkey_mgr,
            self.processor.hwnd,
            &shortcuts.cur_mouse_jump_next,
            ShortcutID::CurMouseJumpNext,
        ) {
            error!("register shortcut cur_mouse_jump_next error: {}", e);
            last_error = Err(e);
        }

        last_error
    }

    fn on_shortcut(&mut self, cb: u32) {
        let id = match self.hotkey_mgr.get_callback(cb) {
            Some(v) => v,
            None => return,
        };
        match id {
            ShortcutID::CurMouseLock => self.on_shortcut_cur_mouse_lock(),
            ShortcutID::CurMouseJumpNext => self.on_shortcut_cur_mouse_jump_next(),
        }
    }

    fn on_shortcut_cur_mouse_lock(&mut self) {
        debug!("Shortcut cur_mouse_lock pressed");
        if self.headless {
            self.processor.cur_mouse_lock_toogle();
            return;
        }
        if let Some(id) = self.processor.devices.active_id() {
            self.mouse_control_reactor
                .ui_tx
                .send(Message::LockCurMouse(id.clone()));
        }
    }

    fn on_shortcut_cur_mouse_jump_next(&mut self) {
        debug!("Shortcut cut_mouse_jump pressed");
        self.processor
            .relocator
            .jump_to_next_monitor(self.processor.devices.active().map(|d| &mut d.ctrl))
    }
}

impl WinEventLoop {
    pub fn new(headless: bool, mouse_control_reactor: MouseControlReactor) -> Self {
        WinHook::set_ev_sender(mouse_control_reactor.mouse_control_tx.clone());
        let hook = WinHook::new();
        let processor = WinDeviceProcessor::new();

        WinEventLoop {
            hook,
            processor,
            headless,
            hotkey_mgr: HotKeyManager::new(),
            mouse_control_reactor,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.setup_window()?;
        self.processor.initialize()?;
        self.hook.register()?;
        Ok(())
    }

    pub fn load_config(&mut self, config: Settings) -> Result<()> {
        self.apply_new_settings(config.processor)
    }

    pub fn terminate(&mut self) -> Result<()> {
        self.hook.unregister()?;
        self.processor.terminate()?;
        Ok(())
    }

    pub fn setup_window(&mut self) -> Result<()> {
        // thread_set_dpi_aware();
        if !process_set_dpi_aware() {
            warn!("Failed to set process as dpi aware");
        };
        let hwnd = match create_dummy_window(None) {
            Ok((_, v)) => v,
            Err(e) => {
                error!("Create dummy window failed: {}", e);
                return Err(e);
            }
        };
        match set_subclass(hwnd, SUBCLASS_UID, Some(self)) {
            Ok(v) => v,
            Err(e) => {
                error!("Set subclass failed: {}", e);
                return Err(e);
            }
        };
        self.processor.hwnd = hwnd;
        Ok(())
    }

    fn handle_wm_message(&mut self, msg: &MSG) {
        match msg.message {
            WM_INPUT => self
                .processor
                .on_raw_input(msg.wParam, msg.lParam, msg.time),
            WM_INPUT_DEVICE_CHANGE => {
                debug!("Trigger updating devices by WM_INPUT_DEVICE_CHANGE");
                self.processor.to_update_devices = true;
            }
            WM_HOTKEY => {
                self.on_shortcut(msg.lParam.0 as u32);
                self.processor.resolve_relocation();
            }
            // And some messages caught by self.subclass_callback()
            _ => (),
        }
    }

    #[inline]
    pub fn poll_wm_messages(&mut self, mut max_events: u32, timeout_ms: u32) -> Result<bool> {
        let mut msg = MSG::default();

        unsafe {
            MsgWaitForMultipleObjects(None, false, timeout_ms, QS_ALLINPUT);
            while max_events > 0
                && PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool()
            {
                if msg.message == WM_QUIT {
                    return Ok(false);
                }
                self.handle_wm_message(&msg);
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
            if !self.poll_wm_messages(
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
    pub fn scan_devices(&mut self) -> Result<Vec<GenericDevice>> {
        match self.processor.try_update_devices(true) {
            Ok(_) => Ok(self
                .processor
                .devices
                .iter()
                .filter(|&v| Self::is_valid_win_device(v))
                .map(Self::win_device_to_generic)
                .collect()),
            Err(e) => Err(e),
        }
    }

    fn apply_new_settings(&mut self, new_settings: ProcessorSettings) -> Result<()> {
        self.processor.apply_processor_settings(Some(new_settings));
        self.register_shortcuts()
    }

    pub fn poll_messages(&mut self) -> bool {
        loop {
            let mut msg = match self.mouse_control_reactor.mouse_control_rx.try_recv() {
                Some(msg) => msg,
                None => return false,
            };

            match &mut msg {
                Message::Exit => {
                    return true;
                }
                Message::SysMouseEvent(mouse_ev) => {
                    self.processor.on_mouse_event(mouse_ev);
                }
                Message::ScanDevices(data) => {
                    data.set_result(self.scan_devices());
                    self.mouse_control_reactor.return_msg(msg)
                }
                Message::InspectDevicesStatus(data) => {
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
                    data.set_ok(ret);
                    self.mouse_control_reactor.return_msg(msg)
                }
                Message::ApplyProcessorSetting(data) => {
                    let req = data.take_req();
                    data.set_result(self.apply_new_settings(req));
                    self.mouse_control_reactor.return_msg(msg)
                }
                Message::ApplyOneDeviceSetting(data) => {
                    let item = data.take();
                    self.processor
                        .devices
                        .update_one_device_settings(&item.id, &item.content);
                }
                _ => panic!("recv unexpected ui msg: {:?}", msg),
            };
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
