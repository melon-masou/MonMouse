use std::collections::BTreeSet;

use log::{debug, error, warn};
use windows::Win32::{
    Foundation::HWND,
    UI::Input::{RAWINPUTDEVICE, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_PAGEONLY},
};

use super::{
    devices::{collect_all_raw_devices, is_valid_win_device, win_device_to_generic},
    winwrap::{
        display::{get_all_monitors_info, process_set_dpi_aware, set_cursor_pos, MonitorInfo},
        input::{register_rawinput_devices, register_shortcut, unregister_shortcut},
        overlay::{
            create_overlay_window, refresh_overlay_window, trigger_draw_cursors, CursorMarker,
            CursorPos,
        },
        window::get_cur_tick,
    },
};
use crate::{
    device_type::WindowsRawinput,
    errors::{Error, Result},
    input_control::{DeviceId, MonitorArea, MonitorAreasList, MousePos},
    processor::{
        effects::CursorDrawing,
        platform::{DeviceInfo, PlatformService},
    },
};

const MAX_SHORTCUT_ID: i32 = 0xbfff;

// Native resources and capabilities; control policy belongs to Processor.
pub struct WinPlatformService {
    pub hwnd: HWND,
    shortcut_ids: BTreeSet<i32>,
    next_shortcut_id: i32,
}

impl Default for WinPlatformService {
    fn default() -> Self {
        Self::new()
    }
}

impl WinPlatformService {
    pub fn new() -> Self {
        Self {
            hwnd: HWND::default(),
            shortcut_ids: BTreeSet::new(),
            next_shortcut_id: 1,
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

    fn register_shortcut(&mut self, text: &str) -> Result<i32> {
        // Avoid immediately reusing an ID that may still be in a queued WM_HOTKEY.
        let Some(id) = (self.next_shortcut_id..=MAX_SHORTCUT_ID)
            .chain(1..self.next_shortcut_id)
            .find(|id| !self.shortcut_ids.contains(id))
        else {
            return Err(Error::ShortcutRegistration(
                "No free Win32 shortcut IDs".into(),
            ));
        };
        register_shortcut(self.hwnd, id, text)?;
        self.shortcut_ids.insert(id);
        self.next_shortcut_id = if id == MAX_SHORTCUT_ID { 1 } else { id + 1 };
        Ok(id)
    }

    fn unregister_shortcut(&mut self, id: i32) -> Result<()> {
        unregister_shortcut(self.hwnd, id)?;
        self.shortcut_ids.remove(&id);
        Ok(())
    }

    fn refresh_overlay_window(&mut self) -> Result<()> {
        refresh_overlay_window(self.hwnd)
    }

    fn draw_cursors(&mut self, drawing: CursorDrawing) {
        let inactive_marker = if drawing.show_inactive_markers {
            CursorMarker::Inactive
        } else {
            CursorMarker::None
        };
        let cursors = drawing
            .inactive
            .into_iter()
            .map(|position| CursorPos {
                position,
                marker: inactive_marker,
            })
            .chain(drawing.injected.map(|position| CursorPos {
                position,
                marker: CursorMarker::Injected,
            }))
            .collect();
        trigger_draw_cursors(self.hwnd, cursors);
    }

    fn set_cursor_pos(&mut self, position: MousePos) -> Result<()> {
        set_cursor_pos(position.x, position.y)
    }

    fn get_cur_tick(&self) -> u64 {
        get_cur_tick()
    }
}
