use config::{Config, File};
use serde::{Deserialize, Serialize};

const DEFAULT_CONFIG_PATH: &str = "conf/monmouse.yml";

pub fn load_config(path: Option<&str>) -> Settings {
    let (path, required) = if let Some(v) = path {
        (v, true)
    } else {
        (DEFAULT_CONFIG_PATH, false)
    };

    let settings = Config::builder()
        .add_source(File::with_name(path).required(required))
        .build()
        .unwrap();

    settings.try_deserialize::<Settings>().unwrap()
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
    pub fn is_effective(d: &DeviceSetting) -> bool {
        d.locked_in_monitor || d.switch
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

// pub struct Settings {
//     pub merge_unassociated_events_within_next_ms: Option<u64>,
//     pub devices: Vec<(String, DeviceSetting)>,
// }

// impl Default for Settings {
//     fn default() -> Self {
//         Self {
//             merge_unassociated_events_within_next_ms: Some(5),
//             devices: Vec::new(),
//         }
//     }
// }

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
