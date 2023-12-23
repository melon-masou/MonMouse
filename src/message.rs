use std::{
    fmt::Debug,
    sync::mpsc::{channel, sync_channel, Receiver, SendError, Sender, SyncSender, TryRecvError},
};

use crate::{device_type::DeviceType, errors::Error, setting::ProcessorSettings};

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

#[derive(Debug)]
pub struct GenericDevice {
    pub id: String,
    pub device_type: DeviceType,
    pub product_name: String,
    pub platform_specific_infos: Vec<(String, String)>,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct SendData<T> {
    inner: Box<Option<T>>,
}

impl<T> SendData<T> {
    pub fn new(d: T) -> Self {
        Self {
            inner: Box::new(Some(d)),
        }
    }
    pub fn take(self) -> T {
        self.inner.unwrap()
    }
}

#[derive(Debug)]
pub struct RoundtripData<TReq, TRsp> {
    inner: Box<(Option<TReq>, Result<TRsp>)>,
}

impl<TReq, TRsp> Default for RoundtripData<TReq, TRsp>
where
    TReq: Default,
{
    #[inline]
    fn default() -> Self {
        RoundtripData::new(TReq::default())
    }
}

impl<TReq, TRsp> RoundtripData<TReq, TRsp> {
    pub fn new(req: TReq) -> Self {
        Self {
            inner: Box::new((Some(req), Err(Error::MessageInited))),
        }
    }

    pub fn req(&self) -> &TReq {
        self.inner.0.as_ref().unwrap()
    }
    pub fn result(&self) -> std::result::Result<&TRsp, &Error> {
        self.inner.1.as_ref()
    }

    pub fn set_result(&mut self, result: Result<TRsp>) {
        self.inner.1 = result;
    }
    pub fn set_ok(&mut self, result: TRsp) {
        self.inner.1 = Ok(result);
    }
    pub fn set_error(&mut self, result: Error) {
        self.inner.1 = Err(result);
    }

    pub fn take_req(&mut self) -> TReq {
        self.inner.0.take().unwrap()
    }
    pub fn take_rsp(self) -> Result<TRsp> {
        self.inner.1
    }
}

#[derive(Debug)]
pub enum Message {
    Exit,
    CloseUI,
    RestartUI,
    LockCurMouse(SendData<String>),
    ScanDevices(RoundtripData<(), Vec<GenericDevice>>),
    InspectDevicesStatus(RoundtripData<(), Vec<(String, DeviceStatus)>>),
    ApplyProcessorSetting(RoundtripData<ProcessorSettings, ()>),
}

#[repr(i32)]
#[derive(Clone, Copy, Debug)]
pub enum ShortcutID {
    CurMouseLock = 1000,
    CurMouseJumpNext = 1001,
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
            Message::ScanDevices(_) => self.ui_tx.send(msg).unwrap(),
            Message::InspectDevicesStatus(_) => self.ui_tx.send(msg).unwrap(),
            Message::ApplyProcessorSetting(_) => self.ui_tx.send(msg).unwrap(),
            _ => panic!("MouseControl should not return msg: {:?}", msg),
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
        panic!("UIReactor should not return msg: {:?}", msg);
    }

    #[inline]
    pub fn send_mouse_control(&self, msg: Message) -> std::result::Result<(), SendError<Message>> {
        self.mouse_control_tx.send(msg)
    }
}
