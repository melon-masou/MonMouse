use std::sync::mpsc::{RecvError, TryRecvError};

use eframe::egui;
use monmouse::message::{DeviceStatus, GenericDevice, Message, UIReactor};

use crate::styles::Theme;

pub struct App {
    pub state: AppState,
    pub last_result: StatusBarResult,
    should_exit: bool,
    ui_reactor: UIReactor,
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
            .send_mouse_control(Message::ScanDevices((), Message::inited()));
    }

    pub fn trigger_inspect_devices_status(&mut self) {
        self.ui_reactor
            .send_mouse_control(Message::InspectDevicesStatus((), Message::inited()));
    }
}

impl App {
    pub fn new(ui_reactor: UIReactor) -> Self {
        App {
            state: AppState::default(),
            last_result: StatusBarResult::None,
            should_exit: false,
            ui_reactor,
        }
    }

    pub fn get_theme(&self) -> Theme {
        Theme::from_string(self.state.global_config.theme.as_str())
    }

    fn merge_scanned_devices(&mut self, mut devs: Vec<GenericDevice>) {
        let mut new_one = Vec::<DeviceUIState>::new();
        while let Some(v) = devs.pop() {
            new_one.push(DeviceUIState {
                locked: false,
                switch: false,
                generic: v,
                status: DeviceStatus::Disconnected,
            });
        }
        self.state.managed_devices = new_one;
    }

    fn update_devices_status(&mut self, mut devs: Vec<(String, DeviceStatus)>) {
        self.state
            .managed_devices
            .iter_mut()
            .for_each(|v| v.status = DeviceStatus::Disconnected);
        while let Some((id, status)) = devs.pop() {
            for d in &mut self.state.managed_devices {
                if d.generic.id == id {
                    d.status = status;
                    break;
                }
            }
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
                Message::ApplyDevicesSetting() => todo!(),
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
    pub found_devices: Vec<DeviceUIState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            global_config: GlobalConfig {
                theme: Theme::Auto.to_string(),
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
    pub status: DeviceStatus,
}

pub enum StatusBarResult {
    Ok(String),
    ErrMsg(String),
    None,
}
