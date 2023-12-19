use std::sync::mpsc::{RecvError, TryRecvError};

use eframe::egui;
use monmouse::{
    message::{DeviceSetting, DeviceStatus, GenericDevice, Message, Settings, UIReactor},
    utils::SimpleRatelimit,
};

use crate::styles::Theme;

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
                Some(self.collect_settings()),
                Message::inited(),
            ))
            .unwrap();
    }
}

impl App {
    pub fn new(ui_reactor: UIReactor) -> Self {
        let state = AppState::default();
        let rl_inspect_devices_status =
            SimpleRatelimit::new(state.global_config.inspect_device_activity_interval_ms);

        App {
            state,
            last_result: StatusBarResult::None,
            should_exit: false,
            ui_reactor,
            rl_inspect_devices_status,
        }
    }

    pub fn get_theme(&self) -> Theme {
        Theme::from_string(self.state.global_config.theme.as_str())
    }

    fn merge_scanned_devices(&mut self, devs: Vec<GenericDevice>) {
        self.state.managed_devices = devs
            .into_iter()
            .map(|v| DeviceUIState {
                locked: false,
                switch: false,
                generic: v,
                status: DeviceStatus::Disconnected,
            })
            .collect();
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

    fn collect_settings(&self) -> Settings {
        Settings {
            merge_unassociated_events_within_next_ms: self
                .state
                .global_config
                .merge_unassociated_events_within_next_ms,
            devices: self
                .state
                .managed_devices
                .iter()
                .map(|d| {
                    (
                        d.generic.id.clone(),
                        DeviceSetting {
                            locked_in_monitor: d.locked,
                            remember_pos: d.switch,
                        },
                    )
                })
                .collect(),
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

pub struct AppState {
    pub global_config: GlobalConfig,
    pub managed_devices: Vec<DeviceUIState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            global_config: GlobalConfig {
                theme: Theme::Auto.to_string(),
                inspect_device_activity_interval_ms: 100,
                merge_unassociated_events_within_next_ms: Some(5),
            },
            managed_devices: Vec::<DeviceUIState>::new(),
        }
    }
}

pub struct GlobalConfig {
    pub theme: String,
    pub inspect_device_activity_interval_ms: u64,
    pub merge_unassociated_events_within_next_ms: Option<u64>,
}

pub struct DeviceUIState {
    pub locked: bool,
    pub switch: bool,
    pub generic: GenericDevice,
    pub status: DeviceStatus,
}

pub enum StatusBarResult {
    Ok(String),
    ErrMsg(String),
    None,
}
