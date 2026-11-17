use std::{collections::HashMap, time::Duration};

use log::{debug, error};

pub(crate) mod message_poller;
mod platform;
pub(crate) use platform::{DeviceInfo, PlatformService};

use crate::input_control::{
    mouse_event_device_matcher::{MatchedMouseEvent, MouseEventDeviceMatcher},
    relocator::{MouseDeviceTracker, MouseRelocator, RelocatePos},
    DeviceId, MonitorAreasList, MousePos, Positioning,
};
use crate::{
    device_type::DeviceType,
    errors::Result,
    message::{
        DeviceStatus, GenericDevice, Message, MessageSender, MouseControlReactor, ShortcutID,
    },
    setting::{DeviceSetting, DeviceSettingItem, ProcessorSettings},
    utils::SimpleRatelimit,
};

pub(crate) const POLL_MAX_MESSAGES: u32 = 20;
pub(crate) const POLL_WAIT_TIMEOUT_MS: u32 = 20;
const STALE_MOUSE_EVENT_MS: u64 = POLL_WAIT_TIMEOUT_MS as u64 * 3;
const MOUSE_EVENT_ACTIVE_LAST_FOR_MS: u64 = 100;
const RATELIMIT_UPDATE_MONITOR_ONCE_MS: u64 = 1000;
const RATELIMIT_UPDATE_DEVICE_ONCE_MS: u64 = 1000;

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

// Coordinates application messages and input decisions through platform capabilities.
// Pollers supply events; this type owns control state, refresh scheduling and effects.
pub(crate) struct Processor {
    devices: Vec<Device>,
    indexs: HashMap<DeviceId, usize>,
    active_id: Option<usize>,
    relocator: MouseRelocator,
    mouse_event_device_matcher: MouseEventDeviceMatcher,
    settings: ProcessorSettings,
    headless: bool,
    to_update_devices: bool,
    to_update_monitors: bool,
    rl_update_mon: SimpleRatelimit,
    rl_update_dev: SimpleRatelimit,
}

impl Processor {
    pub(crate) fn new(headless: bool) -> Self {
        let settings = ProcessorSettings::default();
        Self {
            devices: Vec::new(),
            indexs: HashMap::new(),
            active_id: None,
            relocator: MouseRelocator::new(),
            mouse_event_device_matcher: MouseEventDeviceMatcher::new(
                settings.merge_unassociated_events_ms,
            ),
            settings,
            headless,
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

    fn active_id(&self) -> Option<&String> {
        self.active_id
            .and_then(|i| self.devices[i].info.id.as_ref())
    }

    fn rebuild(&mut self, devices: Vec<Device>) {
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

    fn update_monitors(&mut self, monitors: MonitorAreasList) {
        self.relocator.update_monitors(monitors);
        self.devices
            .iter_mut()
            .for_each(|device| device.ctrl.reset());
    }

    fn inactive_positions(&self) -> Vec<(MousePos, usize)> {
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
                d.ctrl.get_last_pos().map(|(_, pos, _)| (pos, i))
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

    fn apply_one_device_setting(&mut self, item: DeviceSettingItem) {
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

    fn jump_to_next_monitor(&mut self) {
        let ctrl = self
            .active_id
            .and_then(|i| self.devices.get_mut(i))
            .map(|d| &mut d.ctrl);
        self.relocator.jump_to_next_monitor(ctrl);
    }

    fn on_mouse_event(&mut self, event: MatchedMouseEvent, platform: &mut impl PlatformService) {
        self.active_id = self.indexs.get(&event.device).copied();
        let Some(device) = self.active_id.and_then(|i| self.devices.get_mut(i)) else {
            self.to_update_devices = true;
            return;
        };
        if self.relocator.on_mouse_event(&event, &mut device.ctrl) {
            self.draw_inactive_mouses(platform);
        }
        self.resolve_relocation(platform);
    }

    pub(crate) fn on_device_activity(
        &mut self,
        source: Option<DeviceId>,
        positioning: Positioning,
        tick: u64,
        platform: &mut impl PlatformService,
    ) {
        let source = self
            .mouse_event_device_matcher
            .observe_device(source, positioning, tick);
        if !self.indexs.contains_key(&source) {
            self.to_update_devices = true;
        }
        self.resolve_pending_updating_task(platform);
    }

    fn inspect_devices_status(&self, now: u64, active_for_ms: u64) -> Vec<(String, DeviceStatus)> {
        self.devices
            .iter()
            .filter_map(|device| {
                let id = device.info.id.as_ref()?;
                let status = Self::build_device_status(device, now, active_for_ms);
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

    pub(crate) fn initialize(&mut self, platform: &mut impl PlatformService) -> Result<()> {
        if let Err(error) = platform.register_raw_devices() {
            error!("Register raw devices failed: {}", error);
            return Err(error);
        }
        // Raw-device registration supplies the initial device-change notifications.
        if let Err(error) = self.try_update_monitors(platform, true) {
            error!("Init monitors info failed: {}", error);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn terminate(&mut self, platform: &mut impl PlatformService) -> Result<()> {
        self.clear_draw_inactive_mouses(platform);
        Ok(())
    }

    pub(crate) fn reinitialize(&mut self, platform: &mut impl PlatformService) {
        // A newly opened UI can replace the raw-input registration.
        if let Err(error) = platform.register_raw_devices() {
            error!("Reinit register raw devices failed: {}", error);
        }
    }

    pub(crate) fn scan_devices(
        &mut self,
        platform: &mut impl PlatformService,
    ) -> Result<Vec<GenericDevice>> {
        self.try_update_devices(platform, true)?;
        Ok(self
            .devices
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
            .collect())
    }

    fn try_update_devices(
        &mut self,
        platform: &mut impl PlatformService,
        must: bool,
    ) -> Result<()> {
        if !must && !self.rl_update_dev.allow(None).0 {
            return Ok(());
        }
        let mut devices = match platform.scan_devices() {
            Ok(devices) => devices,
            Err(error) => {
                error!("Collect all raw devices failed: {}", error);
                return Err(error);
            }
        };
        // Unidentified events need their own control state when they cannot be merged.
        devices.push(DeviceInfo {
            source: 0,
            id: Some("UnassociatedEventsCapture".into()),
            device_type: DeviceType::Dummy,
            product_name: "UnassociatedEventsCapture".into(),
            platform_specific_infos: Vec::new(),
        });
        debug!("Updated rawdevices list: num={}", devices.len());
        self.rebuild(devices.into_iter().map(Device::new).collect());
        self.apply_processor_settings(None);
        self.draw_inactive_mouses(platform);
        self.to_update_devices = false;
        Ok(())
    }

    fn try_update_monitors(
        &mut self,
        platform: &mut impl PlatformService,
        must: bool,
    ) -> Result<()> {
        if let Err(error) = platform.refresh_overlay_window() {
            error!("refresh overlay window failed: {}", error);
        }
        if !must && !self.rl_update_mon.allow(None).0 {
            return Ok(());
        }
        let monitors = match platform.get_monitors() {
            Ok(monitors) => monitors,
            Err(error) => {
                error!("Update monitors info failed: {}", error);
                return Err(error);
            }
        };
        debug!("Updated monitors: {}", monitors);
        self.update_monitors(monitors);
        self.clear_draw_inactive_mouses(platform);
        self.to_update_monitors = false;
        Ok(())
    }

    pub(crate) fn apply_new_settings(
        &mut self,
        settings: ProcessorSettings,
        platform: &mut impl PlatformService,
    ) -> Result<()> {
        self.apply_processor_settings(Some(settings));
        self.draw_inactive_mouses(platform);
        self.register_shortcuts(platform)
    }

    fn register_shortcuts(&mut self, platform: &mut impl PlatformService) -> Result<()> {
        let shortcuts = &self.settings.shortcuts;
        let mut last_error: Result<()> = Ok(());

        if let Err(e) =
            platform.register_shortcut(&shortcuts.cur_mouse_lock, ShortcutID::CurMouseLock)
        {
            error!("register shortcut cur_mouse_lock error: {}", e);
            last_error = Err(e);
        }

        if let Err(e) =
            platform.register_shortcut(&shortcuts.cur_mouse_switch, ShortcutID::CurMouseSwitch)
        {
            error!("register shortcut cur_mouse_switch error: {}", e);
            last_error = Err(e);
        }

        if let Err(e) =
            platform.register_shortcut(&shortcuts.cur_mouse_jump_next, ShortcutID::CurMouseJumpNext)
        {
            error!("register shortcut cur_mouse_jump_next error: {}", e);
            last_error = Err(e);
        }

        last_error
    }

    pub(crate) fn on_message(
        &mut self,
        mut message: Message,
        platform: &mut impl PlatformService,
        reactor: &MouseControlReactor,
    ) -> bool {
        match &mut message {
            Message::Exit => return true,
            Message::Reinitilization => self.reinitialize(platform),
            Message::MousePosition { position, tick } => {
                if platform.get_cur_tick().saturating_sub(*tick) > STALE_MOUSE_EVENT_MS {
                    return false;
                }
                if let Some(event) = self
                    .mouse_event_device_matcher
                    .match_position(*position, *tick)
                {
                    self.on_mouse_event(event, platform);
                }
            }
            Message::ScanDevices(data) => {
                data.set_result(self.scan_devices(platform));
                reactor.return_msg(message);
            }
            Message::InspectDevicesStatus(data) => {
                data.set_ok(self.inspect_devices_status(
                    platform.get_cur_tick(),
                    MOUSE_EVENT_ACTIVE_LAST_FOR_MS,
                ));
                reactor.return_msg(message);
            }
            Message::ApplyProcessorSetting(data) => {
                let result = self.apply_new_settings(data.req().proc_settings.clone(), platform);
                data.set_result(result);
                reactor.return_msg(message);
            }
            Message::ApplyOneDeviceSetting(data) => self.apply_one_device_setting(data.take()),
            _ => panic!("recv unexpected ui msg: {:?}", message),
        }
        false
    }

    pub(crate) fn on_devices_changed(&mut self) {
        self.to_update_devices = true;
    }

    pub(crate) fn on_monitors_changed(&mut self) {
        self.to_update_monitors = true;
    }

    pub(crate) fn resolve_pending_updating_task(&mut self, platform: &mut impl PlatformService) {
        if self.relocator.pop_need_update_monitors() {
            self.to_update_monitors = true;
        }
        if self.to_update_devices {
            let _ = self.try_update_devices(platform, false);
        }
        if self.to_update_monitors {
            let _ = self.try_update_monitors(platform, false);
        }
    }

    fn draw_inactive_mouses(&mut self, platform: &mut impl PlatformService) {
        let positions = if self.settings.show_inactive_cursors {
            self.inactive_positions()
        } else {
            Vec::new()
        };
        platform.draw_cursors(positions, self.settings.show_inactive_cursor_markers);
    }

    fn clear_draw_inactive_mouses(&mut self, platform: &mut impl PlatformService) {
        platform.draw_cursors(Vec::new(), self.settings.show_inactive_cursor_markers);
    }

    fn resolve_relocation(&mut self, platform: &mut impl PlatformService) {
        if let Some(RelocatePos(position)) = self.relocator.pop_relocate_pos() {
            let _ = platform.set_cursor_pos(position);
            debug!("Reset cursor to ({},{})", position.x, position.y);
        }
    }

    pub(crate) fn on_shortcut(
        &mut self,
        id: Option<ShortcutID>,
        platform: &mut impl PlatformService,
        ui_tx: &MessageSender,
    ) {
        match id {
            Some(ShortcutID::CurMouseLock) => {
                debug!("Shortcut cur_mouse_lock pressed");
                if self.headless {
                    self.cur_mouse_lock_toogle();
                } else if let Some(id) = self.active_id() {
                    ui_tx.send(Message::LockCurMouse(id.clone()));
                }
            }
            Some(ShortcutID::CurMouseSwitch) => {
                debug!("Shortcut cur_mouse_switch pressed");
                if self.headless {
                    self.cur_mouse_switch_toogle();
                } else if let Some(id) = self.active_id() {
                    ui_tx.send(Message::SwitchCurMouse(id.clone()));
                }
            }
            Some(ShortcutID::CurMouseJumpNext) => {
                debug!("Shortcut cut_mouse_jump pressed");
                self.jump_to_next_monitor();
            }
            None => (),
        }
        self.resolve_relocation(platform);
    }
}
