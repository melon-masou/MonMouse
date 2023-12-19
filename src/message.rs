use std::{
    fmt::{Debug, Display},
    sync::mpsc::{channel, sync_channel, Receiver, SendError, Sender, SyncSender, TryRecvError},
};

use crate::{device_type::DeviceType, errors::Error};

#[derive(Debug, Clone, Copy)]
pub enum Positioning {
    Unknown,
    Relative,
    Absolute,
}

#[derive(Debug)]
pub enum DeviceStatus {
    Active(Positioning),
    Idle,
    Disconnected,
    Unknown,
}

pub struct GenericDevice {
    pub id: String,
    pub device_type: DeviceType,
    pub product_name: String,
    pub platform_specific_infos: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceSetting {
    pub locked_in_monitor: bool,
    pub remember_pos: bool,
}

impl Display for DeviceSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DeviceSetting{{locked={},remember={}}}",
            self.locked_in_monitor, self.remember_pos
        )
    }
}

pub struct Settings {
    pub merge_unassociated_events_within_next_ms: Option<u64>,
    pub devices: Vec<(String, DeviceSetting)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            merge_unassociated_events_within_next_ms: Some(5),
            devices: Vec::new(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Message {
    Exit,
    CloseUI,
    RestartUI,
    ScanDevices((), Result<Vec<GenericDevice>>),
    InspectDevicesStatus((), Result<Vec<(String, DeviceStatus)>>),
    ApplyDevicesSetting(Option<Settings>, Result<()>),
}

impl Message {
    #[inline]
    pub fn inited<T>() -> Result<T> {
        Err(Error::MessageInited)
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exit => write!(f, "Msg(Exit)"),
            Self::CloseUI => write!(f, "Msg(CloseUI)"),
            Self::RestartUI => write!(f, "Msg(RestartUI)"),
            Self::ScanDevices(_, _) => write!(f, "Msg(ScanDevices)"),
            Self::InspectDevicesStatus(_, _) => write!(f, "Msg(InspectDevicesStatus)"),
            Self::ApplyDevicesSetting(_, _) => write!(f, "Msg(ApplyDevicesSetting)"),
        }
    }
}

impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct SignalSender(SyncSender<()>);

impl SignalSender {
    pub fn send(&self) {
        let _ = self.0.try_send(());
    }
}

pub struct SignalReceiver(Receiver<()>);

impl SignalReceiver {
    pub fn check(&self) -> Option<bool> {
        match self.0.try_recv() {
            Ok(_) => Some(true),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(false),
        }
    }
}

pub fn signal() -> (SignalSender, SignalReceiver) {
    let (tx, rx) = sync_channel::<()>(1);
    (SignalSender(tx), SignalReceiver(rx))
}

pub fn setup_reactors() -> (MasterReactor, MouseControlReactor, UIReactor) {
    let (ui_tx, ui_rx) = channel::<Message>();
    let (mouse_control_tx, mouse_control_rx) = channel::<Message>();

    let master = MasterReactor {
        ui_tx: ui_tx.clone(),
    };
    let mouse_ctrl = MouseControlReactor {
        ui_tx,
        mouse_control_rx,
    };
    let ui = UIReactor {
        ui_rx,
        mouse_control_tx,
    };

    (master, mouse_ctrl, ui)
}

pub struct MasterReactor {
    ui_tx: Sender<Message>,
}

impl MasterReactor {
    pub fn exit(&self) {
        self.ui_tx.send(Message::CloseUI).unwrap(); // close ui firstly
        self.ui_tx.send(Message::Exit).unwrap();
    }
    pub fn restart_ui(&self) {
        self.ui_tx.send(Message::RestartUI).unwrap();
    }
    pub fn close_ui(&self) {
        self.ui_tx.send(Message::CloseUI).unwrap();
    }
}

pub struct MouseControlReactor {
    pub ui_tx: Sender<Message>,
    pub mouse_control_rx: Receiver<Message>,
}

impl MouseControlReactor {
    #[inline]
    pub fn return_msg(&self, msg: Message) {
        match msg {
            Message::Exit => drop(msg),
            Message::CloseUI => drop(msg),
            Message::RestartUI => drop(msg),
            Message::ScanDevices(_, _) => self.ui_tx.send(msg).unwrap(),
            Message::InspectDevicesStatus(_, _) => self.ui_tx.send(msg).unwrap(),
            Message::ApplyDevicesSetting(_, _) => self.ui_tx.send(msg).unwrap(),
        }
    }
}

pub struct UIReactor {
    pub ui_rx: Receiver<Message>,
    pub mouse_control_tx: Sender<Message>,
}

impl UIReactor {
    #[inline]
    pub fn return_msg(&self, msg: Message) {
        match msg {
            Message::Exit => drop(msg),
            Message::CloseUI => drop(msg),
            Message::RestartUI => drop(msg),
            Message::ScanDevices(_, _) => panic!("return self-generated msg"),
            Message::InspectDevicesStatus(_, _) => panic!("return self-generated msg"),
            Message::ApplyDevicesSetting(_, _) => panic!("return self-generated msg"),
        }
    }

    #[inline]
    pub fn send_mouse_control(&self, msg: Message) -> std::result::Result<(), SendError<Message>> {
        self.mouse_control_tx.send(msg)
    }
}
