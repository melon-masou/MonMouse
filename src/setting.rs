use crate::errors::Error;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

pub const CONFIG_FILE_NAME: &str = "monmouse.yml";

pub fn read_config(file: &PathBuf) -> Result<Settings, Error> {
    match std::fs::read_to_string(file) {
        Ok(v) => Ok(v),
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                Err(Error::ConfigFileNotExists(format!("{}", file.display())))
            }
            _ => Err(Error::IO(e)),
        },
    }
    .and_then(|content| match serde_yaml::from_str::<Settings>(&content) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::InvalidConfigFile(e.to_string())),
    })
}

pub fn write_config(file: &PathBuf, settings: &Settings) -> Result<(), Error> {
    match serde_yaml::to_string(settings) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::InvalidConfigFile(e.to_string())),
    }
    .and_then(|content| match std::fs::write(file, content) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::IO(e)),
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

    #[serde(default = "ShortcutSettings::default")]
    pub shortcuts: ShortcutSettings,
}

impl Default for ProcessorSettings {
    fn default() -> Self {
        Self {
            merge_unassociated_events_ms: Self::default_merge_unassociated_events_ms(),
            devices: Self::default_devices(),
            shortcuts: ShortcutSettings::default(),
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

    pub fn mut_device<R>(
        &mut self,
        id: &str,
        mut f: impl FnMut(&mut DeviceSetting) -> R,
    ) -> Option<R> {
        self.devices
            .iter_mut()
            .find(|d| d.id.as_str() == id)
            .map(|d| f(&mut d.content))
    }
    pub fn ensure_mut_device<R>(
        &mut self,
        id: &str,
        mut f: impl FnMut(&mut DeviceSetting) -> R,
    ) -> R {
        if let Some(r) = self.mut_device(id, &mut f) {
            return r;
        }
        self.devices.push(DeviceSettingItem {
            id: id.to_owned(),
            content: DeviceSetting::default(),
        });
        f(self.devices.last_mut().map(|d| &mut d.content).unwrap())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShortcutSettings {
    #[serde(default = "empty_string")]
    pub cur_mouse_lock: String,

    #[serde(default = "empty_string")]
    pub cur_mouse_jump_next: String,
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

    #[serde(default = "bool_const::<false>")]
    pub hide_ui_on_launch: bool,
}

impl Default for UISettings {
    fn default() -> Self {
        Self {
            theme: Self::default_theme(),
            inspect_device_interval_ms: Self::default_inspect_device_interval_ms(),
            hide_ui_on_launch: false,
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
#[allow(dead_code)]
fn empty_string() -> String {
    "".to_owned()
}
