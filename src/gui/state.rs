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
            managed_devices: vec![DeviceUIState::default(); 20],
            found_devices: vec![DeviceUIState::default(); 6],
        }
    }
}

pub struct GlobalConfig {
    pub theme: String,
}

#[derive(Default, Clone)]
pub struct DeviceUIState {
    pub checked: bool,
    pub locked: bool,
    pub switch: bool,
}
