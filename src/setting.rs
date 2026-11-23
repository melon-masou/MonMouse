use crate::errors::Error;
use monmouse_lib::plugin::PluginKey;
use monmouse_macros::InputFields;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use validator::Validate;

pub mod validation;

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

pub fn config_string(settings: &Settings) -> Result<String, Error> {
    match serde_yaml::to_string(settings) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::InvalidConfigFile(e.to_string())),
    }
}

fn plugin_settings_path(config_dir: Option<&Path>, key: PluginKey) -> Option<PathBuf> {
    config_dir.map(|dir| dir.join(format!("monmouse-plugins-{key}.yaml")))
}

pub fn load_plugin_settings(
    config_dir: Option<&Path>,
    key: PluginKey,
) -> Result<Option<String>, String> {
    let Some(path) = plugin_settings_path(config_dir, key) else {
        return Ok(None);
    };
    match fs::read_to_string(&path) {
        Ok(yaml) => Ok(Some(yaml)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

pub fn save_plugin_settings(
    config_dir: Option<&Path>,
    key: PluginKey,
    yaml: &str,
) -> Result<(), String> {
    let path =
        plugin_settings_path(config_dir, key).ok_or("Plugin settings path is unavailable")?;
    let dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let write = || -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let mut temporary = NamedTempFile::new_in(dir)?;
        temporary.write_all(yaml.as_bytes())?;
        temporary.persist(&path)?;
        Ok(())
    };
    write().map_err(|error| format!("{}: {error}", path.display()))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Validate, InputFields)]
pub struct Settings {
    #[serde(default)]
    #[validate(nested)]
    pub ui: UISettings,

    #[serde(default)]
    #[validate(nested)]
    pub processor: ProcessorSettings,

    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub enabled_plugins: std::collections::BTreeSet<String>,

    // Unknown configuration sections remain opaque.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_yaml::Value>,
}

impl Settings {
    pub fn example() -> Self {
        let mut c = Self::default();
        c.processor.shortcuts.cur_mouse_lock = "Ctrl+Alt+A".to_owned();
        c.processor.shortcuts.cur_mouse_switch = "".to_owned();
        c.processor.shortcuts.cur_mouse_jump_next = "Ctrl+Alt+Q".to_owned();
        c.processor.devices.push(DeviceSettingItem {
            id: "Device1".to_owned(),
            content: DeviceSetting {
                locked_in_monitor: false,
                switch: true,
            },
        });
        c
    }
}

// Settings for single device
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, Validate, InputFields)]
pub struct DeviceSetting {
    #[serde(default = "bool_const::<false>")]
    pub locked_in_monitor: bool,
    #[serde(default = "bool_const::<false>")]
    pub switch: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Validate, InputFields)]
pub struct DeviceSettingItem {
    pub id: String,
    #[serde(flatten)]
    #[validate(nested)]
    pub content: DeviceSetting,
}

impl DeviceSetting {
    pub fn is_effective(&self) -> bool {
        self.locked_in_monitor || self.switch
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Validate, InputFields)]
pub struct MappingRule {
    #[validate(custom(function = "validation::shortcut::<false>"))]
    pub from: String,
    #[validate(custom(function = "validation::shortcut::<true>"))]
    pub to: String,
    // None sends input locally; Some addresses a plugin interceptor callback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_target: Option<SendTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendTarget {
    pub plugin: String,
    pub callback: String,
}

#[derive(Clone, Debug)]
pub struct SendTargetOption {
    pub target: SendTarget,
    pub name: String,
}

// Settings for processor
#[derive(Clone, Debug, Serialize, Deserialize, Validate, InputFields)]
pub struct ProcessorSettings {
    #[serde(default = "ProcessorSettings::default_merge_unassociated_events_ms")]
    #[validate(range(min = -1, max = 1000))]
    pub merge_unassociated_events_ms: i64, // -1 means disabling merging

    #[serde(default = "ProcessorSettings::default_show_inactive_cursors")]
    pub show_inactive_cursors: bool,

    #[serde(default = "ProcessorSettings::default_show_inactive_cursor_markers")]
    pub show_inactive_cursor_markers: bool,

    #[serde(default)]
    pub draw_injected_cursor: bool,

    #[serde(default = "ProcessorSettings::default_devices")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[validate(nested)]
    pub devices: Vec<DeviceSettingItem>,

    #[serde(default = "ShortcutSettings::default")]
    #[validate(nested)]
    pub shortcuts: ShortcutSettings,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(nested)]
    pub mappings: Vec<MappingRule>,
}

impl Default for ProcessorSettings {
    fn default() -> Self {
        Self {
            merge_unassociated_events_ms: Self::default_merge_unassociated_events_ms(),
            show_inactive_cursors: Self::default_show_inactive_cursors(),
            show_inactive_cursor_markers: Self::default_show_inactive_cursor_markers(),
            draw_injected_cursor: false,
            devices: Self::default_devices(),
            shortcuts: ShortcutSettings::default(),
            mappings: Vec::new(),
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
    fn default_show_inactive_cursors() -> bool {
        false
    }
    fn default_show_inactive_cursor_markers() -> bool {
        false
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, Validate, InputFields)]
pub struct ShortcutSettings {
    #[serde(default = "empty_string")]
    #[validate(custom(function = "validation::shortcut::<false>"))]
    pub cur_mouse_freeze: String,

    #[serde(default = "empty_string")]
    #[validate(custom(function = "validation::shortcut::<false>"))]
    pub cur_mouse_lock: String,

    #[serde(default = "empty_string")]
    #[validate(custom(function = "validation::shortcut::<false>"))]
    pub cur_mouse_switch: String,

    #[serde(default = "empty_string")]
    #[validate(custom(function = "validation::shortcut::<false>"))]
    pub cur_mouse_jump_next: String,
}

// Settings for UI
#[derive(Clone, Debug, Serialize, Deserialize, Validate, InputFields)]
pub struct UISettings {
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default = "UISettings::default_theme")]
    pub theme: String,

    #[serde(default = "UISettings::default_language")]
    pub language: String,

    #[serde(default = "UISettings::default_inspect_device_interval_ms")]
    #[validate(range(min = 20, max = 1000))]
    pub inspect_device_interval_ms: u64,

    #[serde(default = "UISettings::default_hide_ui_on_launch")]
    pub hide_ui_on_launch: bool,

    #[serde(default)]
    pub start_on_login: bool,
}

impl Default for UISettings {
    fn default() -> Self {
        Self {
            theme: Self::default_theme(),
            language: Self::default_language(),
            inspect_device_interval_ms: Self::default_inspect_device_interval_ms(),
            hide_ui_on_launch: Self::default_hide_ui_on_launch(),
            start_on_login: false,
        }
    }
}

impl UISettings {
    fn default_theme() -> String {
        "".to_owned()
    }
    fn default_language() -> String {
        "en".to_owned()
    }
    fn default_inspect_device_interval_ms() -> u64 {
        100
    }
    fn default_hide_ui_on_launch() -> bool {
        false
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
