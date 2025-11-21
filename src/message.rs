use std::{
    fmt::Debug,
    sync::mpsc::{
        channel, sync_channel, Receiver, RecvError, RecvTimeoutError, Sender, SyncSender,
        TryRecvError,
    },
    time::Duration,
};

use crate::{
    device_type::DeviceType,
    errors::Error,
    setting::{DeviceSettingItem, ProcessorSettings},
};

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

impl GenericDevice {
    pub fn id_only(id: String) -> GenericDevice {
        GenericDevice {
            id: id.clone(),
            device_type: DeviceType::Unknown,
            product_name: id,
            platform_specific_infos: Vec::new(),
        }
    }
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
    pub fn take(&mut self) -> T {
        self.inner.take().unwrap()
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

#[derive(Clone, Copy, Debug)]
pub enum TimerDueKind {
    InspectDevice,
}

#[derive(Debug)]
pub enum Message {
    Exit,
    RestartUI,
    TimerDue(TimerDueKind),
    LockCurMouse(String),
    ScanDevices(RoundtripData<(), Vec<GenericDevice>>),
    InspectDevicesStatus(RoundtripData<(), Vec<(String, DeviceStatus)>>),
    ApplyProcessorSetting(RoundtripData<ApplyProcessorSettingsData, ApplyProcessorSettingsCtx>),
    ApplyOneDeviceSetting(SendData<DeviceSettingItem>),
    SysMouseEvent(SysMouseEvent),
}

#[derive(Debug)]
pub struct ApplyProcessorSettingsData {
    pub ctx: ApplyProcessorSettingsCtx,
    pub proc_settings: ProcessorSettings,
}

#[derive(Debug)]
pub struct ApplyProcessorSettingsCtx {
    pub user_requested: bool,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug)]
pub enum ShortcutID {
    CurMouseLock = 1000,
    CurMouseJumpNext = 1001,
}

#[derive(Clone, Copy, Debug)]
pub struct SysMouseEvent {
    pub pos_x: i32,
    pub pos_y: i32,
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

pub fn setup_reactors(
    ui_notify1: Box<dyn UINotify>,
    ui_notify2: Box<dyn UINotify>,
) -> (TrayReactor, MouseControlReactor, UIReactor) {
    let (ui_tx, ui_rx) = channel::<Message>();
    let (mouse_control_tx, mouse_control_rx) = channel::<Message>();

    let tray = TrayReactor {
        ui_tx: MessageSender::from(&ui_tx),
        mouse_control_tx: MessageSender::from(&mouse_control_tx),
        ui_notify: ui_notify1,
    };
    let mouse_ctrl = MouseControlReactor {
        ui_tx: MessageSender::from(&ui_tx),
        mouse_control_tx: MessageSender::from(&mouse_control_tx.clone()),
        mouse_control_rx: MessageReceiver::from(mouse_control_rx),
        ui_notify: ui_notify2,
    };
    let ui = UIReactor {
        ui_rx: MessageReceiver::from(ui_rx),
        ui_tx: MessageSender::from(&ui_tx),
        mouse_control_tx: MessageSender::from(&mouse_control_tx),
    };

    (tray, mouse_ctrl, ui)
}

pub struct TrayReactor {
    ui_tx: MessageSender,
    mouse_control_tx: MessageSender,
    ui_notify: Box<dyn UINotify>,
}

impl TrayReactor {
    pub fn exit(&self) {
        self.ui_notify.notify_close();
        self.ui_tx.send(Message::Exit);
        self.mouse_control_tx.send(Message::Exit);
    }
    pub fn restart_ui(&self) {
        self.ui_tx.send(Message::RestartUI);
    }
}

pub struct UIReactor {
    pub ui_rx: MessageReceiver,
    pub ui_tx: MessageSender,
    pub mouse_control_tx: MessageSender,
}

pub struct MouseControlReactor {
    pub ui_tx: MessageSender,
    pub mouse_control_tx: MessageSender,
    pub mouse_control_rx: MessageReceiver,
    ui_notify: Box<dyn UINotify>,
}

impl MouseControlReactor {
    #[inline]
    pub fn return_msg(&self, msg: Message) {
        match msg {
            Message::ScanDevices(_) => {
                self.ui_tx.send(msg);
                self.ui_notify.notify();
            }
            Message::InspectDevicesStatus(_) => {
                self.ui_tx.send(msg);
                self.ui_notify.notify();
            }
            Message::ApplyProcessorSetting(_) => {
                self.ui_tx.send(msg);
                self.ui_notify.notify();
            }
            _ => panic!("MouseControl should not return msg: {:?}", msg),
        }
    }
}

#[derive(Debug)]
pub struct MessageReceiver(Receiver<Message>);

impl MessageReceiver {
    fn from(r: Receiver<Message>) -> Self {
        Self(r)
    }

    #[inline]
    pub fn try_recv(&self) -> Option<Message> {
        match self.0.try_recv() {
            Ok(v) => Some(v),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Message::Exit),
        }
    }

    #[inline]
    pub fn recv(&self) -> Message {
        match self.0.recv() {
            Ok(v) => v,
            Err(RecvError) => Message::Exit,
        }
    }

    #[inline]
    pub fn recv_timeout(&self, dur: Duration) -> Option<Message> {
        match self.0.recv_timeout(dur) {
            Ok(v) => Some(v),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => Some(Message::Exit),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MessageSender(Sender<Message>);

impl MessageSender {
    fn from(s: &Sender<Message>) -> Self {
        Self(s.clone())
    }

    #[inline]
    pub fn send(&self, msg: Message) {
        let _ = self.0.send(msg);
    }
}

pub trait UINotify: Send {
    fn notify(&self);
    fn notify_close(&self);
}

#[derive(Clone, Default)]
pub struct UINotifyNoop {}

impl UINotify for UINotifyNoop {
    fn notify(&self) {}
    fn notify_close(&self) {}
}

pub enum TimerOperation {
    ResetInterval(Duration),
}

pub struct TimerOperator {
    op_tx: Sender<TimerOperation>,
    handle: std::thread::JoinHandle<()>,
}

impl TimerOperator {
    pub fn update_interval(&self, dur: Duration) {
        let _ = self.op_tx.send(TimerOperation::ResetInterval(dur));
    }
    pub fn stop(self) {
        drop(self.op_tx);
        let _ = self.handle.join();
    }
}

pub fn timer_spawn(
    mut interval: Duration,
    tx: MessageSender,
    kind: TimerDueKind,
    callback: Option<Box<dyn Fn() + Send>>,
) -> TimerOperator {
    let (op_tx, op_rx) = channel::<TimerOperation>();

    let handle = std::thread::spawn(move || loop {
        loop {
            match op_rx.try_recv() {
                Ok(o) => match o {
                    TimerOperation::ResetInterval(d) => interval = d,
                },
                Err(TryRecvError::Disconnected) => return,
                _ => break,
            }
        }
        std::thread::sleep(interval);
        tx.send(Message::TimerDue(kind));
        if let Some(cb) = &callback {
            cb()
        }
    });

    TimerOperator { op_tx, handle }
}
