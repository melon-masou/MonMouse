use std::time::Duration;

use log::{debug, error};

use super::{effects::EffectsIntent, platform::PlatformService, Processor};
use crate::{
    errors::Result,
    message::{GenericDevice, Message, MessageBus},
    setting::ProcessorSettings,
    utils::SimpleRatelimit,
};

pub enum DispatchResult {
    Continue,
    Exit,
}

const RATELIMIT_UPDATE_MONITOR_ONCE_MS: u64 = 1000;
const RATELIMIT_UPDATE_DEVICE_ONCE_MS: u64 = 1000;

// Native callbacks record separate effects; Poller merges them at event boundaries.
pub struct MessageDispatch {
    rl_update_mon: SimpleRatelimit,
    rl_update_dev: SimpleRatelimit,
    pub effects: EffectsIntent,
}

impl Default for MessageDispatch {
    fn default() -> Self {
        Self {
            rl_update_mon: SimpleRatelimit::new(
                Duration::from_millis(RATELIMIT_UPDATE_MONITOR_ONCE_MS),
                None,
            ),
            rl_update_dev: SimpleRatelimit::new(
                Duration::from_millis(RATELIMIT_UPDATE_DEVICE_ONCE_MS),
                None,
            ),
            effects: EffectsIntent::default(),
        }
    }
}

impl MessageDispatch {
    pub fn scan_devices(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) -> Result<Vec<GenericDevice>> {
        self.refresh_devices(processor, platform)?;
        Ok(processor.devices())
    }

    fn check_refresh_devices(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) -> Result<()> {
        if !self.effects.refresh_devices || !self.rl_update_dev.allow(None).0 {
            return Ok(());
        }
        self.refresh_devices(processor, platform)
    }
    fn refresh_devices(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) -> Result<()> {
        let devices = match platform.scan_devices() {
            Ok(devices) => devices,
            Err(error) => {
                error!("Collect all raw devices failed: {}", error);
                return Err(error);
            }
        };
        processor.update_devices(devices);
        self.effects.refresh_devices = false;
        self.effects.redraw_inactive_cursors = true;
        Ok(())
    }

    fn check_refresh_monitors(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) -> Result<()> {
        if !self.effects.refresh_monitors || !self.rl_update_mon.allow(None).0 {
            return Ok(());
        }
        self.refresh_monitors(processor, platform)
    }
    pub fn refresh_monitors(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) -> Result<()> {
        let monitors = match platform.get_monitors() {
            Ok(monitors) => monitors,
            Err(error) => {
                error!("Update monitors info failed: {}", error);
                return Err(error);
            }
        };
        debug!("Updated monitors: {}", monitors);
        processor.update_monitors(monitors, &mut self.effects);
        self.effects.refresh_overlay = true;
        self.effects.refresh_monitors = false;
        Ok(())
    }

    pub fn apply_settings(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
        settings: &ProcessorSettings,
    ) -> Result<()> {
        self.effects.redraw_inactive_cursors = true; // May enable/disable cursors drawing

        processor.apply_new_settings(settings.clone());
        platform.register_shortcuts(&settings.shortcuts)
    }

    pub fn dispatch(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
        mut message: Message,
        bus: &MessageBus,
    ) -> DispatchResult {
        match &mut message {
            Message::Exit => return DispatchResult::Exit,
            Message::Reinitilization => {
                // A newly opened UI can replace the raw-input registration.
                if let Err(error) = platform.register_raw_devices() {
                    error!("Reinit register raw devices failed: {}", error);
                }
            }
            Message::ScanDevices(data) => {
                data.set_result(self.scan_devices(processor, platform));
                bus.ui.send(message);
            }
            Message::InspectDevicesStatus(data) => {
                data.set_ok(processor.inspect_devices_status(platform.get_cur_tick()));
                bus.ui.send(message);
            }
            Message::ApplyProcessorSetting(data) => {
                // Native registration errors still prevent a successful save acknowledgement.
                let result = self.apply_settings(processor, platform, &data.req().proc_settings);
                data.set_result(result);
                bus.ui.send(message);
            }
            Message::ApplyOneDeviceSetting(data) => processor.apply_one_device_setting(data.take()),
            other => unreachable!("unexpected message routed to mouse control: {other:?}"),
        }
        DispatchResult::Continue
    }

    pub fn resolve_effects(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) {
        // Failed or rate-limited refreshes remain pending for the next resolution.
        let _ = self.check_refresh_devices(processor, platform);
        let _ = self.check_refresh_monitors(processor, platform);
    }
}
