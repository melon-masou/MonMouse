use log::{debug, error, warn};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::Input::{RAWINPUTDEVICE, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_PAGEONLY},
};

use super::{
    constants::SUBCLASS_UID,
    devices::{collect_all_raw_devices, is_valid_win_device, win_device_to_generic},
    winwrap::{
        display::{get_all_monitors_info, process_set_dpi_aware, set_cursor_pos, MonitorInfo},
        input::{register_rawinput_devices, HotKeyManager},
        overlay::{create_overlay_window, refresh_overlay_window, trigger_draw_cursors, CursorPos},
        window::{get_cur_tick, set_subclass, SubclassHandler},
    },
};
use crate::{
    device_type::WindowsRawinput,
    errors::{Error, Result},
    input_control::{DeviceId, MonitorArea, MonitorAreasList, MousePos},
    keyboard::key_windows::shortcut_str_to_win,
    message::ShortcutID,
    processor::{DeviceInfo, PlatformService},
};

// Native resources and capabilities; control policy belongs to Processor.
pub(super) struct WinPlatformService {
    hwnd: HWND,
    pub(super) hotkey_mgr: HotKeyManager<ShortcutID>,
    pub(super) monitors_changed: bool,
}

impl SubclassHandler for WinPlatformService {
    fn subclass_callback(&mut self, message: u32, _wp: WPARAM, _lp: LPARAM, _class: usize) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{WM_DISPLAYCHANGE, WM_DPICHANGED};
        match message {
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                debug!("Trigger updating monitors by WM {}", message);
                self.monitors_changed = true;
            }
            _ => (),
        }
        true
    }
}

impl WinPlatformService {
    pub(super) fn new() -> Self {
        Self {
            hwnd: HWND::default(),
            hotkey_mgr: HotKeyManager::new(),
            monitors_changed: false,
        }
    }

    pub(super) fn setup_window(&mut self) -> Result<()> {
        // thread_set_dpi_aware();
        if !process_set_dpi_aware() {
            warn!("Failed to set process as dpi aware");
        };
        let hwnd = match create_overlay_window(None) {
            Ok((_, v)) => v,
            Err(e) => {
                error!("Create overlay window failed: {}", e);
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
        self.hwnd = hwnd;
        Ok(())
    }

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

    fn filter_monitor(mon: &MonitorInfo) -> bool {
        mon.rect.left < mon.rect.right && mon.rect.top < mon.rect.bottom
    }

    fn monitor_area_from(mi: &MonitorInfo) -> MonitorArea {
        MonitorArea::new(
            MousePos::from(mi.rect.left, mi.rect.top),
            MousePos::from(mi.rect.right, mi.rect.bottom),
        )
    }
}

impl PlatformService for WinPlatformService {
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

    fn scan_devices(&mut self) -> Result<Vec<DeviceInfo>> {
        Ok(collect_all_raw_devices()?
            .into_iter()
            .map(|device| {
                debug!("Device: {}", device);
                let (product_name, platform_specific_infos) = if is_valid_win_device(&device) {
                    let info = win_device_to_generic(&device);
                    (info.product_name, info.platform_specific_infos)
                } else {
                    (String::new(), Vec::new())
                };
                DeviceInfo {
                    source: device.handle.0 as DeviceId,
                    id: device.id,
                    device_type: device.device_type,
                    product_name,
                    platform_specific_infos,
                }
            })
            .collect())
    }

    fn get_monitors(&mut self) -> Result<MonitorAreasList> {
        let monitors = get_all_monitors_info()?;
        self.monitors_changed = false;
        Ok(MonitorAreasList::from(
            monitors
                .iter()
                .filter(|mon| Self::filter_monitor(mon))
                .map(Self::monitor_area_from)
                .collect(),
        ))
    }

    fn register_shortcut(&mut self, shortcut: &str, id: ShortcutID) -> Result<()> {
        Self::apply_one_shortcut(&mut self.hotkey_mgr, self.hwnd, shortcut, id)
    }

    fn refresh_overlay_window(&mut self) -> Result<()> {
        refresh_overlay_window(self.hwnd)
    }

    fn draw_cursors(&mut self, positions: Vec<(MousePos, usize)>, show_marker: bool) {
        let cursors = positions
            .into_iter()
            .map(|(pos, id)| CursorPos(pos.x, pos.y, id))
            .collect();
        trigger_draw_cursors(self.hwnd, cursors, show_marker);
    }

    fn set_cursor_pos(&mut self, position: MousePos) -> Result<()> {
        set_cursor_pos(position.x, position.y)
    }

    fn get_cur_tick(&self) -> u64 {
        get_cur_tick()
    }
}
