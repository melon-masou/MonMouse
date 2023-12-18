use monmouse::message::GenericDevice;

use crate::styles::Theme;

pub struct AppState {
    pub global_config: GlobalConfig,
    pub managed_devices: Vec<DeviceUIState>,
    pub found_devices: Vec<DeviceUIState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            global_config: GlobalConfig {
                theme: Theme::Light.to_string(),
            },
            managed_devices: Vec::<DeviceUIState>::new(),
            found_devices: Vec::<DeviceUIState>::new(),
        }
    }
}

pub struct GlobalConfig {
    pub theme: String,
}

pub struct DeviceUIState {
    pub locked: bool,
    pub switch: bool,
    pub generic: GenericDevice,
}
