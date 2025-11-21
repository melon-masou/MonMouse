use std::{path::PathBuf, time::Duration};

use monmouse::{
    errors::Error,
    message::{
        timer_spawn, DeviceStatus, GenericDevice, Message, RoundtripData, SendData, TimerDueKind,
        TimerOperator, UINotify, UIReactor,
    },
    setting::{write_config, DeviceSetting, DeviceSettingItem, ProcessorSettings, Settings},
};

use crate::{components::config_panel::ConfigInputState, styles::Theme, EguiNotify};

pub struct App {
    pub state: AppState,
    pub last_result: StatusBarResult,
    pub alert_errors: Vec<String>,
    config_path: Option<PathBuf>,
    should_exit: bool,
    ui_reactor: UIReactor,
    inspect_timer: Option<TimerOperator>,
}

impl App {
    pub fn trigger_scan_devices(&mut self) {
        self.result_clear();
        self.ui_reactor
            .mouse_control_tx
            .send(Message::ScanDevices(RoundtripData::default()));
    }

    pub fn trigger_inspect_devices_status(&mut self) {
        self.ui_reactor
            .mouse_control_tx
            .send(Message::InspectDevicesStatus(RoundtripData::default()));
    }

    pub fn trigger_one_device_setting_changed(&mut self, item: DeviceSettingItem) {
        self.ui_reactor
            .mouse_control_tx
            .send(Message::ApplyOneDeviceSetting(SendData::new(item)));
    }

    pub fn trigger_settings_changed(&mut self) {
        self.result_clear();
        self.ui_reactor
            .mouse_control_tx
            .send(Message::ApplyProcessorSetting(RoundtripData::new(
                self.collect_processor_settings(),
            )));
    }

    pub fn setup_inspect_timer(&mut self, egui_notify: &EguiNotify) {
        let egui_notify = egui_notify.clone();
        let timer = timer_spawn(
            Duration::from_millis(self.state.settings.ui.inspect_device_interval_ms),
            self.ui_reactor.ui_tx.clone(),
            TimerDueKind::InspectDevice,
            Some(Box::new(move || egui_notify.notify())),
        );
        self.inspect_timer = Some(timer);
    }

    pub fn on_settings_applied(&mut self) {
        self.state.config_input.mark_changed(false);
    }
    pub fn apply_new_settings(&mut self) {
        match self.state.config_input.set_into(&mut self.state.settings) {
            Ok(_) => {
                let duration =
                    Duration::from_millis(self.state.settings.ui.inspect_device_interval_ms);
                if let Some(timer) = self.inspect_timer.as_ref() {
                    timer.update_interval(duration);
                }
                self.trigger_settings_changed();
            }
            Err(_) => self.result_error_alert("Not all fields contain valid value".to_owned()),
        }
    }
    pub fn restore_settings(&mut self) {
        self.state.config_input.set_from(&self.state.settings);
        self.result_ok("Settings restored".to_owned());
    }
    pub fn set_default_settings(&mut self) {
        self.state.config_input.set_from(&Settings::default());
        self.result_ok("Default settings restored".to_owned());
    }
}

impl App {
    pub fn new(ui_reactor: UIReactor) -> Self {
        App {
            state: AppState::default(),
            last_result: StatusBarResult::None,
            alert_errors: Vec::new(),
            config_path: None,
            should_exit: false,
            ui_reactor,
            inspect_timer: None,
        }
    }

    pub fn load_config(
        mut self,
        config: Result<Settings, Error>,
        config_path: Option<PathBuf>,
    ) -> Self {
        match config {
            Ok(s) => {
                self.init_managed_devices(&s.processor);
                self.state.settings = s.clone();
                self.state.saved_settings = s;
            }
            Err(Error::ConfigFileNotExists(_)) => (),
            Err(e) => {
                self.result_error_alert(format!("Cannot load config, use default config: {}", e))
            }
        };
        self.state.config_input.set_from(&self.state.settings);
        self.config_path = config_path;
        self
    }

    pub fn get_theme(&self) -> Theme {
        Theme::from_string(self.state.settings.ui.theme.as_str())
    }

    fn init_managed_devices(&mut self, settings: &ProcessorSettings) {
        for dev in &settings.devices {
            self.state.managed_devices.push(DeviceUIState {
                device_setting: dev.content,
                generic: GenericDevice::id_only(dev.id.clone()),
                status: DeviceStatus::Disconnected,
            })
        }
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

    pub fn on_launch_wait_start_ui<T: Fn()>(&mut self, fn_before_wait: T) -> bool {
        if !self.state.settings.ui.hide_ui_on_launch {
            return self.should_exit;
        }
        fn_before_wait();
        self.wait_for_restart_background()
    }

    pub fn wait_for_restart_background(&mut self) -> bool /* exit? */ {
        if self.should_exit {
            return true;
        }
        // Once clearing residual pending msg
        loop {
            match self.ui_reactor.ui_rx.try_recv() {
                Some(Message::Exit) => return true,
                Some(msg) => {
                    // Handle others msg normally
                    self.handle_message(msg);
                }
                None => break,
            }
        }
        // Actuall wait for restart msg
        loop {
            match self.ui_reactor.ui_rx.recv() {
                Message::Exit => return true,
                Message::RestartUI => return false,
                msg => {
                    // Handle others msg normally
                    self.handle_message(msg);
                }
            }
        }
    }

    pub fn poll_messages(&mut self) {
        while let Some(msg) = self.ui_reactor.ui_rx.try_recv() {
            self.handle_message(msg)
        }
    }

    pub fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Exit => {
                self.should_exit = true;
            }
            Message::RestartUI => (),
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
                    .mouse_control_tx
                    .send(Message::ApplyOneDeviceSetting(SendData::new(
                        DeviceSettingItem {
                            id,
                            content: dev.device_setting,
                        },
                    )));
            }
            Message::ScanDevices(data) => match data.take_rsp() {
                Ok(devs) => {
                    let dev_num = devs.len();
                    self.merge_scanned_devices(devs);
                    self.result_ok(format!("Scanned {} devices", dev_num))
                }
                Err(e) => self.result_error_alert(format!("Failed to scan devices: {}", e)),
            },
            Message::TimerDue(TimerDueKind::InspectDevice) => self.trigger_inspect_devices_status(),
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
            .map(|d| d.clone_setting())
            .collect();
        self.state.settings.processor.devices = new_settings.processor.devices.clone();
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
        // Don't write the whole new_settings into state.settings, since only one of global/devices config is to be saved.
        // self.state.settings = new_settings;
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

impl DeviceUIState {
    pub fn clone_setting(&self) -> DeviceSettingItem {
        DeviceSettingItem {
            id: self.generic.id.clone(),
            content: self.device_setting,
        }
    }
}

pub enum StatusBarResult {
    Ok(String),
    ErrMsg(String),
    None,
}
