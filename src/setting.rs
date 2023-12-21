use crate::errors::Error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CONFIG_FILE_NAME: &str = "monmouse.yml";

pub fn read_config(file: PathBuf) -> Result<Settings, Error> {
    match std::fs::read_to_string(file) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::CannotOpenConfig(e.to_string())),
    }
    .and_then(|content| match serde_yaml::from_str::<Settings>(&content) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::InvalidConfigFile(e.to_string())),
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub ui: UISettings,
    #[serde(default)]
    pub processor: ProcessorSettings,
}

// Settings for single device
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct DeviceSetting {
    #[serde(default = "bool_const::<false>")]
    pub locked_in_monitor: bool,
    #[serde(default = "bool_const::<false>")]
    pub switch: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceSettingItem {
    pub id: String,
    #[serde(flatten)]
    pub content: DeviceSetting,
}

impl DeviceSetting {
    pub fn is_effective(&self) -> bool {
        self.locked_in_monitor || self.switch
    }
}

// Settings for processor
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessorSettings {
    #[serde(default = "ProcessorSettings::default_merge_unassociated_events_ms")]
    pub merge_unassociated_events_ms: i64,

    #[serde(default = "ProcessorSettings::default_devices")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<DeviceSettingItem>,
}

impl Default for ProcessorSettings {
    fn default() -> Self {
        Self {
            merge_unassociated_events_ms: Self::default_merge_unassociated_events_ms(),
            devices: Self::default_devices(),
        }
    }
}

impl ProcessorSettings {
    fn default_merge_unassociated_events_ms() -> i64 {
        5
    }
    fn default_devices() -> Vec<DeviceSettingItem> {
        Vec::new()
    }
}

// Settings for UI
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UISettings {
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "UISettings::default_theme")]
    pub theme: String,

    #[serde(default = "UISettings::default_inspect_device_interval_ms")]
    pub inspect_device_interval_ms: u64,
}

impl Default for UISettings {
    fn default() -> Self {
        Self {
            theme: Self::default_theme(),
            inspect_device_interval_ms: Self::default_inspect_device_interval_ms(),
        }
    }
}

impl UISettings {
    fn default_theme() -> String {
        "".to_owned()
    }
    fn default_inspect_device_interval_ms() -> u64 {
        100
    }
}

// Some helper functions for serde_derive default
#[allow(dead_code)]
const fn u64_const<const V: u64>() -> u64 {
    V
}
#[allow(dead_code)]
const fn i64_const<const V: i64>() -> i64 {
    V
}
#[allow(dead_code)]
const fn bool_const<const V: bool>() -> bool {
    V
}
