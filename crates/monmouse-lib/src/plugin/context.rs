use super::{CustomMessage, PluginMessage, PluginTarget, SendTarget};
use crate::{input::InputInjection, keyboard::Button};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

pub trait MainContext {
    fn send(&self, target: PluginTarget, message: PluginMessage) -> bool;

    // Queues a shortcut update. Calls after init receive a ShortcutRegistrationResult event.
    fn register_shortcuts(&self, bindings: Vec<(String, String)>) -> Result<(), String>;
    fn config_dir(&self) -> Option<&Path>;
    fn load_settings_yaml(&self) -> Result<Option<String>, String>;
    fn save_settings_yaml(&self, yaml: &str) -> Result<(), String>;
    // Show the host's save confirmation after a successful user-requested save.
    fn report_settings_saved(&self);
    fn report_error(&self, text: &str, alert: bool);

    fn register_send_targets(&self, targets: Vec<SendTarget>) -> Result<(), String>;
}

impl dyn MainContext + '_ {
    pub fn send_msg(&self, target: PluginTarget, message: impl CustomMessage) -> bool {
        self.send(target, PluginMessage::custom(message))
    }

    pub fn load_settings<T: DeserializeOwned>(&self) -> Result<Option<T>, String> {
        self.load_settings_yaml()?
            .as_deref()
            .map(serde_yaml::from_str)
            .transpose()
            .map_err(|error| error.to_string())
    }

    pub fn save_settings<T: Serialize>(&self, value: &T) -> Result<(), String> {
        let yaml = serde_yaml::to_string(value).map_err(|error| error.to_string())?;
        self.save_settings_yaml(&yaml)
    }
}

pub trait ReactorContext {
    fn send(&self, target: PluginTarget, message: PluginMessage) -> bool;
    fn inject_input(&self, input: InputInjection, extra_info: usize) -> Result<(), String>;
    fn inject_shortcut(&self, shortcut: &[Button], down: bool) -> Result<(), String>;
}

impl dyn ReactorContext + '_ {
    pub fn send_msg(&self, target: PluginTarget, message: impl CustomMessage) -> bool {
        self.send(target, PluginMessage::custom(message))
    }
}
