use std::collections::BTreeSet;

use log::{debug, error, warn};
use windows::Win32::{
    Foundation::HWND,
    UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, VIRTUAL_KEY},
    UI::Input::{RAWINPUTDEVICE, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_PAGEONLY},
};

use super::{
    devices::{collect_all_raw_devices, is_valid_win_device, win_device_to_generic},
    winwrap::{
        display::{get_all_monitors_info, process_set_dpi_aware, set_cursor_pos, MonitorInfo},
        input::{register_rawinput_devices, HotKeyManager},
        overlay::{create_overlay_window, refresh_overlay_window, trigger_draw_cursors, CursorPos},
        window::get_cur_tick,
    },
};
use crate::{
    device_type::WindowsRawinput,
    errors::{Error, Result},
    input_control::{DeviceId, MonitorArea, MonitorAreasList, MousePos},
    keyboard::key_windows::shortcut_str_to_win,
    message::ShortcutID,
    processor::platform::{DeviceInfo, PlatformService},
    setting::ShortcutSettings,
};

struct ShortcutBinding {
    action: ShortcutID,
    modifiers: HOT_KEY_MODIFIERS,
    key: VIRTUAL_KEY,
    text: String,
}

// Native resources and capabilities; control policy belongs to Processor.
pub struct WinPlatformService {
    pub hwnd: HWND,
    pub hotkey_mgr: HotKeyManager<ShortcutID>,
    shortcuts: Vec<ShortcutBinding>,
}

impl WinPlatformService {
    pub fn new() -> Self {
        Self {
            hwnd: HWND::default(),
            hotkey_mgr: HotKeyManager::new(),
            shortcuts: Vec::new(),
        }
    }

    pub fn setup_window(&mut self) -> Result<()> {
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
        self.hwnd = hwnd;
        Ok(())
    }

    fn set_bindings(
        manager: &mut HotKeyManager<ShortcutID>,
        hwnd: HWND,
        bindings: &[ShortcutBinding],
    ) -> Result<()> {
        manager.unregister_all(hwnd)?;
        for binding in bindings {
            manager
                .register(
                    hwnd,
                    binding.action as i32,
                    binding.modifiers,
                    binding.key,
                    false,
                    binding.action,
                )
                .map_err(|error| match error {
                    Error::ShortcutConflict(_) => {
                        Error::ShortcutConflict(binding.text.as_str().into())
                    }
                    other => other,
                })?;
        }
        Ok(())
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
        Ok(MonitorAreasList::from(
            monitors
                .iter()
                .filter(|mon| Self::filter_monitor(mon))
                .map(Self::monitor_area_from)
                .collect(),
        ))
    }

    fn register_shortcuts(&mut self, shortcuts: &ShortcutSettings) -> Result<()> {
        let mut bindings = Vec::new();
        let mut chords = BTreeSet::new();
        for (action, text) in [
            (ShortcutID::CurMouseLock, &shortcuts.cur_mouse_lock),
            (ShortcutID::CurMouseSwitch, &shortcuts.cur_mouse_switch),
            (ShortcutID::CurMouseJumpNext, &shortcuts.cur_mouse_jump_next),
        ] {
            if text.is_empty() {
                continue;
            }
            let (modifiers, key) =
                shortcut_str_to_win(text).ok_or_else(|| Error::InvalidShortcut(text.clone()))?;
            if !chords.insert((modifiers.0, key.0)) {
                return Err(Error::ShortcutConflict(text.as_str().into()));
            }
            bindings.push(ShortcutBinding {
                action,
                modifiers,
                key,
                text: text.clone(),
            });
        }
        if let Err(error) = Self::set_bindings(&mut self.hotkey_mgr, self.hwnd, &bindings) {
            Self::set_bindings(&mut self.hotkey_mgr, self.hwnd, &self.shortcuts)?;
            return Err(error);
        }
        self.shortcuts = bindings;
        Ok(())
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
