pub mod effects;
pub mod message_dispatch;
pub mod platform;
pub mod shortcuts;

use self::{
    effects::{CursorDrawing, EffectsIntent},
    platform::DeviceInfo,
};

use std::collections::HashMap;

use log::debug;

use crate::input_control::{
    mouse_event_device_matcher::MouseEventDeviceMatcher,
    relocator::{MouseDeviceTracker, MouseRelocator, RelocatorEffect},
    DeviceId, MatchedMouseEvent, MonitorAreasList, MouseAction, MousePos, Positioning,
};
use crate::{
    device_type::DeviceType,
    message::{DeviceStatus, GenericDevice, Message, ShortcutID},
    plugin::interceptor::InterceptorRegistry,
    setting::{DeviceSetting, DeviceSettingItem, ProcessorSettings},
};

const MOUSE_EVENT_ACTIVE_LAST_FOR_MS: u64 = 100;
const INJECTED_CURSOR_IDLE_MS: u64 = 5_000;

struct Device {
    info: DeviceInfo,
    ctrl: MouseDeviceTracker,
}

impl Device {
    fn new(info: DeviceInfo) -> Self {
        let ctrl = MouseDeviceTracker::new(
            info.source,
            DeviceSetting {
                locked_in_monitor: false,
                switch: false,
            },
        );
        Self { info, ctrl }
    }
}

// Business state and input decisions. Pending native work is recorded in the
// caller's EffectsIntent and executed after state borrows have ended.
pub struct Processor {
    devices: Vec<Device>,
    indexs: HashMap<DeviceId, usize>,
    active_id: Option<usize>, // Index of MonMouse's current device
    relocator: MouseRelocator,
    mouse_event_device_matcher: MouseEventDeviceMatcher,
    injected_cursor: Option<(MousePos, u64)>,
    pub interceptors: InterceptorRegistry,
    settings: ProcessorSettings,
    headless: bool,
}

impl Processor {
    pub fn new(headless: bool) -> Self {
        let settings = ProcessorSettings::default();
        Self {
            devices: Vec::new(),
            indexs: HashMap::new(),
            active_id: None,
            relocator: MouseRelocator::new(),
            mouse_event_device_matcher: MouseEventDeviceMatcher::new(
                settings.merge_unassociated_events_ms,
            ),
            injected_cursor: None,
            interceptors: InterceptorRegistry::default(),
            settings,
            headless,
        }
    }

    fn active_id(&self) -> Option<&String> {
        self.active_id
            .and_then(|i| self.devices[i].info.id.as_ref())
    }

    pub fn monitors(&self) -> &MonitorAreasList {
        self.relocator.monitors()
    }

    pub fn update_monitors(&mut self, monitors: MonitorAreasList, effects: &mut EffectsIntent) {
        if self.monitors() != &monitors {
            self.injected_cursor = None;
        }
        self.relocator.update_monitors(monitors);
        self.devices
            .iter_mut()
            .for_each(|device| device.ctrl.reset());
        // The pending target was calculated against the previous monitor layout.
        effects.relocation = None;
        effects.redraw_cursors = true;
    }

    fn inactive_positions(&self) -> Vec<MousePos> {
        self.devices
            .iter()
            .enumerate()
            .filter_map(|(i, d)| {
                if Some(i) == self.active_id {
                    return None;
                }
                if d.info.device_type.is_dummy() || !d.ctrl.setting.switch {
                    return None;
                }
                d.ctrl.get_last_pos().map(|(_, pos, _)| pos)
            })
            .collect()
    }

    fn cur_mouse_lock_toogle(&mut self) {
        let device = self.active_id.and_then(|i| self.devices.get_mut(i));
        let Some(device) = device else {
            return;
        };
        let Some(id) = &device.info.id else {
            return;
        };
        let content = self.settings.ensure_mut_device(id, |d| {
            d.locked_in_monitor = !d.locked_in_monitor;
            *d
        });
        device.ctrl.update_settings(&content);
    }

    fn cur_mouse_switch_toogle(&mut self) {
        let device = self.active_id.and_then(|i| self.devices.get_mut(i));
        let Some(device) = device else {
            return;
        };
        let Some(id) = &device.info.id else {
            return;
        };
        let content = self.settings.ensure_mut_device(id, |d| {
            d.switch = !d.switch;
            *d
        });
        device.ctrl.update_settings(&content);
    }

    pub fn apply_one_device_setting(&mut self, item: DeviceSettingItem) {
        self.settings
            .ensure_mut_device(&item.id, |d| *d = item.content);
        if let Some(device) = self
            .devices
            .iter_mut()
            .find(|d| d.info.id.as_ref() == Some(&item.id))
        {
            device.ctrl.update_settings(&item.content);
        }
    }

    fn apply_processor_settings(&mut self, new_settings: Option<ProcessorSettings>) {
        if let Some(new) = new_settings {
            self.settings = new;
        }
        let settings = &self.settings;
        self.mouse_event_device_matcher
            .set_merge_unassociated_ms(settings.merge_unassociated_events_ms);
        if !settings.draw_injected_cursor {
            self.injected_cursor = None;
        }
        let mut applied = 0;
        for item in &settings.devices {
            if let Some(device) = self
                .devices
                .iter_mut()
                .find(|d| d.info.id.as_ref() == Some(&item.id))
            {
                device.ctrl.update_settings(&item.content);
                applied += 1;
            }
        }

        debug!(
            "{} in {} devices setting has not been applied",
            applied,
            settings.devices.len()
        );
    }

    fn jump_to_next_monitor(&mut self, tick: u64, effects: &mut EffectsIntent) {
        let ctrl = self
            .active_id
            .and_then(|i| self.devices.get_mut(i))
            .map(|d| &mut d.ctrl);
        let effect = self.relocator.jump_to_next_monitor(ctrl);
        Self::collect_relocator_effects(effects, effect, tick);
    }

    // Returns whether the matched input event should be passed through.
    pub fn on_mouse_event(
        &mut self,
        event: MatchedMouseEvent,
        action: Option<MouseAction>,
        effects: &mut EffectsIntent,
    ) -> bool {
        self.active_id = self.indexs.get(&event.device).copied();
        let Some(device) = self.active_id.and_then(|i| self.devices.get_mut(i)) else {
            effects.refresh_devices = true;
            return true;
        };
        let effect = self.relocator.on_mouse_event(&event, &mut device.ctrl);
        let allow_event = !effect.block_movement || action != Some(MouseAction::Move);
        Self::collect_relocator_effects(effects, effect, event.tick);
        allow_event
    }

    fn collect_relocator_effects(effects: &mut EffectsIntent, effect: RelocatorEffect, tick: u64) {
        effects.redraw_cursors |= effect.switch;
        effects.refresh_monitors |= effect.refresh_monitors;
        if let Some(position) = effect.relocate_pos {
            effects.relocate(position, tick);
        }
    }

    // Input consumption updates state and pending work without calling the platform.
    pub fn match_mouse_position(&self, position: MousePos, tick: u64) -> Option<MatchedMouseEvent> {
        self.mouse_event_device_matcher
            .match_position(position, tick)
    }

    pub fn on_device_activity(
        &mut self,
        source: Option<DeviceId>,
        positioning: Positioning,
        tick: u64,
        effects: &mut EffectsIntent,
    ) {
        let source = self
            .mouse_event_device_matcher
            .observe_device(source, positioning, tick);
        if !self.indexs.contains_key(&source) {
            effects.refresh_devices = true;
        }
    }

    pub fn inspect_devices_status(&self, now: u64) -> Vec<(String, DeviceStatus)> {
        self.devices
            .iter()
            .filter_map(|device| {
                let id = device.info.id.as_ref()?;
                let status = Self::build_device_status(device, now, MOUSE_EVENT_ACTIVE_LAST_FOR_MS);
                Some((id.clone(), status))
            })
            .collect()
    }

    fn build_device_status(device: &Device, now: u64, active_for_ms: u64) -> DeviceStatus {
        if let Some((last_tick, _, positioning)) = device.ctrl.get_last_pos() {
            if last_tick + active_for_ms > now {
                DeviceStatus::Active(positioning)
            } else {
                DeviceStatus::Idle
            }
        } else {
            DeviceStatus::Idle
        }
    }

    pub fn devices(&self) -> Vec<GenericDevice> {
        self.devices
            .iter()
            .filter_map(|device| {
                let info = &device.info;
                Some(GenericDevice {
                    id: info.id.clone()?,
                    device_type: info.device_type,
                    product_name: info.product_name.clone(),
                    platform_specific_infos: info.platform_specific_infos.clone(),
                })
            })
            .collect()
    }

    fn rebuild_devices_list(&mut self, mut devices: Vec<Device>) {
        for device in &mut devices {
            let Some(id) = &device.info.id else {
                continue;
            };
            if let Some(previous) = self
                .devices
                .iter()
                .find(|previous| previous.info.id.as_ref() == Some(id))
            {
                device.ctrl.inherit_runtime_settings(&previous.ctrl);
            }
        }
        self.devices = devices;
        self.indexs = self
            .devices
            .iter()
            .enumerate()
            .map(|(i, device)| (device.ctrl.id, i))
            .collect();
        self.active_id = None;
        self.mouse_event_device_matcher.reset();
    }

    pub fn update_devices(&mut self, mut devices: Vec<DeviceInfo>) {
        // Unidentified events need their own control state when they cannot be merged.
        devices.push(DeviceInfo {
            source: 0,
            id: Some("UnassociatedEventsCapture".into()),
            device_type: DeviceType::Dummy,
            product_name: "UnassociatedEventsCapture".into(),
            platform_specific_infos: Vec::new(),
        });
        debug!("Updated rawdevices list: num={}", devices.len());
        self.rebuild_devices_list(devices.into_iter().map(Device::new).collect());
        self.apply_processor_settings(None);
    }

    pub fn apply_new_settings(&mut self, settings: ProcessorSettings) {
        self.apply_processor_settings(Some(settings));
    }

    pub fn observe_injected_cursor(
        &mut self,
        position: MousePos,
        tick: u64,
        effects: &mut EffectsIntent,
    ) {
        if self.settings.draw_injected_cursor {
            if !effects.redraw_cursors {
                effects.redraw_cursors |=
                    self.injected_cursor.map(|(prev, _)| prev) != Some(position);
            }
            self.injected_cursor = Some((position, tick));
        }
    }

    pub fn expire_injected_cursor(&mut self, now: u64, effects: &mut EffectsIntent) {
        if self
            .injected_cursor
            .is_some_and(|(_, tick)| now.saturating_sub(tick) >= INJECTED_CURSOR_IDLE_MS)
        {
            self.injected_cursor = None;
            effects.redraw_cursors = true;
        }
    }

    pub fn cursor_drawing(&self) -> CursorDrawing {
        CursorDrawing {
            inactive: if self.settings.show_inactive_cursors {
                self.inactive_positions()
            } else {
                Vec::new()
            },
            show_inactive_markers: self.settings.show_inactive_cursor_markers,
            injected: self.injected_cursor.map(|(position, _)| position),
        }
    }

    pub fn on_shortcut(
        &mut self,
        id: Option<ShortcutID>,
        tick: u64,
        effects: &mut EffectsIntent,
    ) -> Option<Message> {
        match id {
            Some(ShortcutID::CurMouseFreeze) => {
                if let Some(device) = self.active_id.and_then(|i| self.devices.get_mut(i)) {
                    device.ctrl.movement_locked = !device.ctrl.movement_locked;
                }
            }
            Some(ShortcutID::CurMouseLock) => {
                debug!("Shortcut cur_mouse_lock pressed");
                if self.headless {
                    self.cur_mouse_lock_toogle();
                } else if let Some(id) = self.active_id() {
                    // Initialize from UI side, which should also modify its settings instance
                    return Some(Message::LockCurMouse(id.clone()));
                }
            }
            Some(ShortcutID::CurMouseSwitch) => {
                debug!("Shortcut cur_mouse_switch pressed");
                if self.headless {
                    self.cur_mouse_switch_toogle();
                } else if let Some(id) = self.active_id() {
                    // Initialize from UI side, which should also modify its settings instance
                    return Some(Message::SwitchCurMouse(id.clone()));
                }
            }
            Some(ShortcutID::CurMouseJumpNext) => {
                debug!("Shortcut cut_mouse_jump pressed");
                self.jump_to_next_monitor(tick, effects);
            }
            None => (),
        }
        None
    }
}
