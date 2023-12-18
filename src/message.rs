use std::{
    fmt::{Debug, Display},
    sync::mpsc::{channel, sync_channel, Receiver, RecvError, Sender, SyncSender, TryRecvError},
};

use crate::errors::Error;

#[derive(Debug)]
pub enum Positioning {
    Unknown,
    Relative,
    Absolute,
}

#[derive(Debug)]
pub enum DeviceStatus {
    Active { positioning: Positioning },
    Idle,
    Disconnected,
}

#[derive(Debug)]
pub enum DeviceType {
    Mouse,
    HIDUnknown,
    Unknown,
}

pub struct GenericDevice {
    pub id: String,
    pub device_type: DeviceType,
    pub product_name: String,
    pub platform_specific_infos: Vec<(String, String)>,
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Message {
    Exit,
    CloseUI,
    RestartUI,
    ScanDevices((), Result<Vec<GenericDevice>>),
    ApplyDevicesSetting(),
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
            Self::ApplyDevicesSetting() => write!(f, "Msg(ApplyDevicesSetting)"),
        }
    }
}

impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // write!(f, "Msg({:?})", std::mem::discriminant(self))
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
            Message::ApplyDevicesSetting() => self.ui_tx.send(msg).unwrap(),
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
            Message::ApplyDevicesSetting() => panic!("return self-generated msg"),
        }
    }

    #[inline]
    pub fn send_mouse_control(&self, msg: Message) {
        self.mouse_control_tx.send(msg).unwrap();
    }
}
