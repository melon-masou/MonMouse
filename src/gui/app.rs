use std::sync::mpsc::{RecvError, TryRecvError};

use eframe::egui;
use monmouse::{
    errors::Error,
    message::{DeviceStatus, GenericDevice, Message, UIReactor},
    setting::{DeviceSetting, DeviceSettingItem, ProcessorSettings, Settings},
    utils::SimpleRatelimit,
};

use crate::{components::config_panel::ConfigInputState, styles::Theme};

pub struct App {
    pub state: AppState,
    pub last_result: StatusBarResult,
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
            .send_mouse_control(Message::ScanDevices((), Message::inited()))
            .unwrap();
    }

    pub fn trigger_inspect_devices_status(&mut self, tick: u64) {
        if self.rl_inspect_devices_status.allow(tick) {
            let _ = self
                .ui_reactor
                .send_mouse_control(Message::InspectDevicesStatus((), Message::inited()));
        }
    }

    pub fn trigger_settings_changed(&mut self) {
        self.result_clear();
        self.ui_reactor
            .send_mouse_control(Message::ApplyDevicesSetting(
                Some(self.collect_processor_settings()),
                Message::inited(),
            ))
            .unwrap();
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
            should_exit: false,
            ui_reactor,
            rl_inspect_devices_status,
        }
    }

    pub fn load_config(mut self, config: Result<Settings, Error>) -> Self {
        match config {
            Ok(s) => self.state.settings = s,
            Err(e) => self.result_error(format!("Cannot load config, use default config: {}", e)),
        }
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
                Message::ScanDevices(_, result) => match result {
                    Ok(devs) => {
                        let dev_num = devs.len();
                        self.merge_scanned_devices(devs);
                        self.result_ok(format!("Scanned {} devices", dev_num))
                    }
                    Err(e) => self.result_error(format!("Failed to scan devices: {}", e)),
                },
                Message::InspectDevicesStatus(_, result) => match result {
                    Ok(devs) => self.update_devices_status(devs),
                    Err(e) => self.result_error(format!("Failed to update device status: {}", e)),
                },
                Message::ApplyDevicesSetting(_, result) => match result {
                    Ok(_) => self.result_ok("New settings applyed".to_owned()),
                    Err(e) => self.result_error(format!("Failed to apply settings: {}", e)),
                },
            }
        }
    }

    fn result_ok(&mut self, msg: String) {
        self.last_result = StatusBarResult::Ok(msg);
    }
    fn result_error(&mut self, msg: String) {
        self.last_result = StatusBarResult::ErrMsg(msg);
    }
    fn result_clear(&mut self) {
        self.last_result = StatusBarResult::None;
    }
}

#[derive(Default)]
pub struct AppState {
    pub settings: Settings,
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
