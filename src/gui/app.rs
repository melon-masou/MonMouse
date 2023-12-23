use std::{
    path::PathBuf,
    sync::mpsc::{RecvError, TryRecvError},
};

use eframe::egui;
use monmouse::{
    errors::Error,
    message::{DeviceStatus, GenericDevice, Message, RoundtripData, SendData, UIReactor},
    setting::{write_config, DeviceSetting, DeviceSettingItem, ProcessorSettings, Settings},
    utils::SimpleRatelimit,
};

use crate::{components::config_panel::ConfigInputState, styles::Theme};

pub struct App {
    pub state: AppState,
    pub last_result: StatusBarResult,
    pub alert_errors: Vec<String>,
    config_path: Option<PathBuf>,
    should_exit: bool,
    ui_reactor: UIReactor,
    rl_inspect_devices_status: SimpleRatelimit,
}

impl App {
    pub fn wait_for_restart(&self) -> bool /* exit? */ {
        if self.should_exit {
            return true;
        }
        // Once clearing residual pending msg
        loop {
            match self.ui_reactor.ui_rx.try_recv() {
                Ok(Message::Exit) => return true,
                Ok(msg) => drop(msg),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
        // Actuall wait for restart msg
        loop {
            match self.ui_reactor.ui_rx.recv() {
                Ok(Message::Exit) => return true,
                Ok(Message::RestartUI) => return false,
                Ok(msg) => drop(msg),
                Err(RecvError) => return true,
            }
        }
    }

    pub fn trigger_scan_devices(&mut self) {
        self.result_clear();
        self.ui_reactor
            .send_mouse_control(Message::ScanDevices(RoundtripData::default()))
            .unwrap();
    }

    pub fn trigger_inspect_devices_status(&mut self, tick: u64) {
        if self.rl_inspect_devices_status.allow(tick) {
            let _ = self
                .ui_reactor
                .send_mouse_control(Message::InspectDevicesStatus(RoundtripData::default()));
        }
    }

    pub fn trigger_settings_changed(&mut self) {
        self.result_clear();
        self.ui_reactor
            .send_mouse_control(Message::ApplyProcessorSetting(RoundtripData::new(
                self.collect_processor_settings(),
            )))
            .unwrap();
    }

    pub fn on_settings_applied(&mut self) {
        self.state.config_input.mark_changed(false);
    }
    pub fn apply_new_settings(&mut self) {
        match self.state.config_input.parse_all(&mut self.state.settings) {
            Ok(_) => {
                self.rl_inspect_devices_status
                    .reset(self.state.settings.ui.inspect_device_interval_ms);
                self.trigger_settings_changed();
            }
            Err(_) => self.result_error_alert("Not all fields contain valid value".to_owned()),
        }
    }
    pub fn restore_settings(&mut self) {
        self.state.config_input.set(&self.state.settings);
        self.result_ok("Settings restored".to_owned());
    }
    pub fn set_default_settings(&mut self) {
        self.state.config_input.set(&Settings::default());
        self.result_ok("Default settings restored".to_owned());
    }
}

impl App {
    pub fn new(ui_reactor: UIReactor) -> Self {
        let state = AppState::default();
        let rl_inspect_devices_status =
            SimpleRatelimit::new(state.settings.ui.inspect_device_interval_ms);

        App {
            state,
            last_result: StatusBarResult::None,
            alert_errors: Vec::new(),
            config_path: None,
            should_exit: false,
            ui_reactor,
            rl_inspect_devices_status,
        }
    }

    pub fn load_config(
        mut self,
        config: Result<Settings, Error>,
        config_path: Option<PathBuf>,
    ) -> Self {
        match config {
            Ok(s) => {
                self.state.settings = s.clone();
                self.state.saved_settings = s;
            }
            Err(Error::ConfigFileNotExists(_)) => (),
            Err(e) => {
                self.result_error_alert(format!("Cannot load config, use default config: {}", e))
            }
        };
        self.state.config_input.set(&self.state.settings);
        self.config_path = config_path;
        self
    }

    pub fn get_theme(&self) -> Theme {
        Theme::from_string(self.state.settings.ui.theme.as_str())
    }

    fn merge_scanned_devices(&mut self, new_devs: Vec<GenericDevice>) {
        // Mark disconnected
        for dev in &mut self.state.managed_devices {
            dev.status = DeviceStatus::Disconnected;
        }
        // Merge list
        for new_dev in new_devs.into_iter() {
            match self
                .state
                .managed_devices
                .iter_mut()
                .find(|v| v.generic.id == new_dev.id)
            {
                Some(dev) => {
                    dev.generic = new_dev;
                    dev.status = DeviceStatus::Idle;
                }
                None => self.state.managed_devices.push(DeviceUIState {
                    device_setting: DeviceSetting::default(),
                    generic: new_dev,
                    status: DeviceStatus::Idle,
                }),
            }
        }
        // Remove disconnected and not managed
        // self.state.managed_devices.retain(|v| {
        //     !matches!(v.status, DeviceStatus::Disconnected) || v.device_setting.is_effective()
        // })
    }

    fn update_devices_status(&mut self, devs: Vec<(String, DeviceStatus)>) {
        self.state
            .managed_devices
            .iter_mut()
            .for_each(|v| v.status = DeviceStatus::Disconnected);

        devs.into_iter().for_each(|(id, status)| {
            for d in &mut self.state.managed_devices {
                if d.generic.id == id {
                    d.status = status;
                    break;
                }
            }
        });
    }

    fn collect_processor_settings(&self) -> ProcessorSettings {
        ProcessorSettings {
            devices: self
                .state
                .managed_devices
                .iter()
                .map(|d| DeviceSettingItem {
                    id: d.generic.id.clone(),
                    content: d.device_setting,
                })
                .collect(),
            shortcuts: self.state.settings.processor.shortcuts.clone(),
            ..self.state.settings.processor
        }
    }

    pub fn dispatch_ui_msg(&mut self, ctx: &egui::Context) {
        loop {
            let msg = match self.ui_reactor.ui_rx.try_recv() {
                Ok(msg) => msg,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.should_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            };

            match msg {
                Message::Exit => {
                    self.should_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Message::CloseUI => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Message::RestartUI => drop(msg),
                Message::LockCurMouse(id) => {
                    let Some(dev) = self
                        .state
                        .managed_devices
                        .iter_mut()
                        .find(|v| v.generic.id == id)
                    else {
                        return;
                    };
                    dev.device_setting.locked_in_monitor = !dev.device_setting.locked_in_monitor;
                    self.ui_reactor
                        .send_mouse_control(Message::ApplyOneDeviceSetting(SendData::new(
                            DeviceSettingItem {
                                id,
                                content: dev.device_setting,
                            },
                        )))
                        .unwrap()
                }
                Message::ScanDevices(data) => match data.take_rsp() {
                    Ok(devs) => {
                        let dev_num = devs.len();
                        self.merge_scanned_devices(devs);
                        self.result_ok(format!("Scanned {} devices", dev_num))
                    }
                    Err(e) => self.result_error_alert(format!("Failed to scan devices: {}", e)),
                },
                Message::InspectDevicesStatus(data) => match data.take_rsp() {
                    Ok(devs) => self.update_devices_status(devs),
                    Err(e) => {
                        self.result_error_silent(format!("Failed to update device status: {}", e))
                    }
                },
                Message::ApplyProcessorSetting(data) => match data.take_rsp() {
                    Ok(_) => {
                        self.result_ok("New settings applyed".to_owned());
                        self.on_settings_applied();
                    }
                    Err(e) => self.result_error_alert(format!("Failed to apply settings: {}", e)),
                },
                #[allow(unreachable_patterns)]
                _ => panic!("recv unexpected msg: {:?}", msg),
            }
        }
    }

    pub fn save_global_config(&mut self) {
        let mut new_settings = self.state.settings.clone();
        new_settings.processor.devices = self.state.saved_settings.processor.devices.clone();
        self.save_config(new_settings);
    }
    pub fn save_devices_config(&mut self) {
        let mut new_settings = self.state.saved_settings.clone();
        new_settings.processor.devices = self
            .state
            .managed_devices
            .iter()
            .filter(|d| d.device_setting.is_effective())
            .map(|d| DeviceSettingItem {
                id: d.generic.id.clone(),
                content: d.device_setting,
            })
            .collect();
        self.save_config(new_settings);
    }
    fn save_config(&mut self, new_settings: Settings) {
        let Some(path) = &self.config_path else {
            self.result_error_alert("No path to save config".to_owned());
            return;
        };
        match write_config(path, &new_settings) {
            Ok(_) => (),
            Err(e) => {
                self.result_error_alert(format!("Failed to write config file: {}", e));
                return;
            }
        }
        self.result_ok("Config saved".to_owned());
        self.state.saved_settings = new_settings.clone();
        self.state.settings = new_settings;
    }

    pub fn result_ok(&mut self, msg: String) {
        self.last_result = StatusBarResult::Ok(msg);
    }
    pub fn result_error_silent(&mut self, msg: String) {
        self.last_result = StatusBarResult::ErrMsg(msg);
    }
    pub fn result_error_alert(&mut self, msg: String) {
        self.alert_errors.push(msg);
    }
    pub fn result_clear(&mut self) {
        self.last_result = StatusBarResult::None;
    }
}

#[derive(Default)]
pub struct AppState {
    pub settings: Settings,
    pub saved_settings: Settings,
    pub managed_devices: Vec<DeviceUIState>,
    pub config_input: ConfigInputState,
}

pub struct DeviceUIState {
    pub device_setting: DeviceSetting,
    pub generic: GenericDevice,
    pub status: DeviceStatus,
}

pub enum StatusBarResult {
    Ok(String),
    ErrMsg(String),
    None,
}
