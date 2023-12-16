use std::{
    fmt::{Debug, Display},
    sync::mpsc::{channel, sync_channel, Receiver, RecvError, Sender, SyncSender, TryRecvError},
};

use crate::errors::Error;

pub enum DeviceStatus {
    Active,
    Idle,
    Disconnected,
}

pub enum Positioning {
    Unknown,
    Relative,
    Absolute,
}

pub enum DeviceType {
    Mouse,
    UnknownHID,
}

pub struct DeviceDetail {
    pub id: String,
    pub device_type: DeviceType,
    pub product_name: String,
    pub active: bool,
    pub positioning: Positioning,
    pub platform_specific_infos: Vec<(String, String)>,
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Message {
    Exit,
    CloseUI,
    RestartUI,
    InspectDevices((), Result<Vec<DeviceDetail>>),
    ApplyDevicesSetting(),
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exit => write!(f, "Msg(Exit)"),
            Self::CloseUI => write!(f, "Msg(CloseUI)"),
            Self::RestartUI => write!(f, "Msg(RestartUI)"),
            Self::InspectDevices(_, _) => write!(f, "Msg(InspectDevices)"),
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
        should_exit: false,
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
    ui_tx: Sender<Message>,
    mouse_control_rx: Receiver<Message>,
}

impl MouseControlReactor {
    #[inline]
    pub fn recv_msg(&self) -> Option<Message> {
        match self.mouse_control_rx.try_recv() {
            Ok(msg) => Some(msg),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn return_msg(msg: Message) {
        match msg {
            Message::Exit => drop(msg),
            Message::CloseUI => drop(msg),
            Message::RestartUI => drop(msg),
            Message::InspectDevices(_, _) => panic!("return self-generated msg"),
            Message::ApplyDevicesSetting() => panic!("return self-generated msg"),
        }
    }
}

pub struct UIReactor {
    ui_rx: Receiver<Message>,
    mouse_control_tx: Sender<Message>,
    should_exit: bool,
}

impl UIReactor {
    #[inline]
    pub fn recv_msg(&self) -> Option<Message> {
        match self.ui_rx.try_recv() {
            Ok(msg) => Some(msg),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn return_msg(&self, msg: Message) {
        match msg {
            Message::Exit => drop(msg),
            Message::CloseUI => drop(msg),
            Message::RestartUI => drop(msg),
            Message::InspectDevices(_, _) => panic!("return self-generated msg"),
            Message::ApplyDevicesSetting() => panic!("return self-generated msg"),
        }
    }

    pub fn wait_for_restart(&self) -> bool /* exit? */ {
        if self.should_exit {
            return true;
        }
        // Once clearing residual pending msg
        loop {
            match self.ui_rx.try_recv() {
                Ok(Message::Exit) => return true,
                Ok(msg) => drop(msg),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
        // Actuall wait for restart msg
        loop {
            match self.ui_rx.recv() {
                Ok(Message::Exit) => return true,
                Ok(Message::RestartUI) => return false,
                Ok(msg) => drop(msg),
                Err(RecvError) => return true,
            }
        }
    }

    pub fn set_should_exit(&mut self) {
        self.should_exit = true;
    }
}
