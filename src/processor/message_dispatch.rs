use std::{collections::BTreeMap, time::Duration};

use log::{debug, error};
use monmouse_lib::plugin::{EventKind, PluginKey, PluginMessage};

use super::{
    effects::EffectsIntent,
    platform::PlatformService,
    shortcuts::{ShortcutAction, ShortcutManager, ShortcutSource},
    Processor,
};
use crate::{
    errors::Result,
    message::{GenericDevice, Message, MessageBus, MessageSender, ShortcutID},
    plugin::PluginSubscriber,
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
    pub shortcuts: ShortcutManager,
    plugin_subscribers: BTreeMap<PluginKey, Vec<PluginSubscriber>>,
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
            shortcuts: ShortcutManager::default(),
            plugin_subscribers: BTreeMap::new(),
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
    ) -> Result<bool> {
        if !self.effects.refresh_monitors || !self.rl_update_mon.allow(None).0 {
            return Ok(false);
        }
        self.refresh_monitors(processor, platform)
    }
    pub fn refresh_monitors(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) -> Result<bool> {
        let monitors = match platform.get_monitors() {
            Ok(monitors) => monitors,
            Err(error) => {
                error!("Update monitors info failed: {}", error);
                return Err(error);
            }
        };
        debug!("Updated monitors: {}", monitors);
        let changed = processor.monitors() != &monitors;
        processor.update_monitors(monitors, &mut self.effects);
        self.effects.refresh_overlay = true;
        self.effects.refresh_monitors = false;
        Ok(changed)
    }

    pub fn apply_settings(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
        settings: &ProcessorSettings,
        user_requested: bool,
    ) -> Result<()> {
        let shortcuts = &settings.shortcuts;
        let bindings = vec![
            (
                shortcuts.cur_mouse_lock.clone(),
                ShortcutAction::Main(ShortcutID::CurMouseLock),
            ),
            (
                shortcuts.cur_mouse_switch.clone(),
                ShortcutAction::Main(ShortcutID::CurMouseSwitch),
            ),
            (
                shortcuts.cur_mouse_jump_next.clone(),
                ShortcutAction::Main(ShortcutID::CurMouseJumpNext),
            ),
        ];
        if let Err(error) = self.shortcuts.register_shortcuts(
            platform,
            ShortcutSource::Main,
            bindings,
            user_requested,
        ) {
            if user_requested {
                return Err(error);
            }
            error!("Could not register startup shortcuts: {error}");
        }
        processor.apply_new_settings(settings.clone());
        self.effects.redraw_inactive_cursors = true;
        Ok(())
    }

    pub fn dispatch(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
        mut message: Message,
        bus: &MessageBus,
    ) -> DispatchResult {
        if let Message::PluginMessage { key, message } = message {
            processor.interceptors.on_event(key, message);
            return DispatchResult::Continue;
        }

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
                let req = data.req();
                let result = self.apply_settings(
                    processor,
                    platform,
                    &req.proc_settings,
                    req.user_requested,
                );
                data.set_result(result);
                bus.ui.send(message);
            }
            Message::ApplyOneDeviceSetting(data) => processor.apply_one_device_setting(data.take()),
            Message::SetPluginInterceptor { key, interceptor } => {
                processor.interceptors.set(key, interceptor.take());
            }
            Message::SetPluginSubscribers { key, subscribers } => {
                if let Some(subscribers) = subscribers.take() {
                    for subscriber in &subscribers {
                        subscriber.publish(
                            EventKind::MonitorChange,
                            key,
                            || PluginMessage::MonitorChange(processor.monitors().clone()),
                            &bus.ui,
                        );
                    }
                    self.plugin_subscribers.insert(key, subscribers);
                } else {
                    self.plugin_subscribers.remove(key);
                }
            }
            Message::RegisterPluginShortcuts {
                key,
                bindings,
                restore_on_error,
            } => {
                let bindings = bindings
                    .drain(..)
                    .map(|(text, action)| (text, ShortcutAction::Plugin(key, action)))
                    .collect();
                let result = self
                    .shortcuts
                    .register_shortcuts(
                        platform,
                        ShortcutSource::Plugin(key),
                        bindings,
                        *restore_on_error,
                    )
                    .map_err(|error| error.to_string());
                if *restore_on_error {
                    bus.ui.send(Message::PluginMessage {
                        key,
                        message: PluginMessage::ShortcutRegistrationResult(result),
                    });
                } else if let Err(text) = result {
                    bus.ui.send(Message::PluginError { text, alert: false });
                }
            }
            other => unreachable!("unexpected message routed to mouse control: {other:?}"),
        }
        DispatchResult::Continue
    }

    pub fn resolve_effects(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
        ui: &MessageSender,
    ) {
        // Failed or rate-limited refreshes remain pending for the next resolution.
        let _ = self.check_refresh_devices(processor, platform);
        if self
            .check_refresh_monitors(processor, platform)
            .unwrap_or(false)
        {
            self.publish_plugin_event(EventKind::MonitorChange, ui, || {
                PluginMessage::MonitorChange(processor.monitors().clone())
            });
        }
    }

    fn publish_plugin_event(
        &self,
        kind: EventKind,
        ui: &MessageSender,
        message: impl Fn() -> PluginMessage,
    ) {
        for (&key, subscribers) in &self.plugin_subscribers {
            for subscriber in subscribers {
                subscriber.publish(kind, key, &message, ui);
            }
        }
    }
}
