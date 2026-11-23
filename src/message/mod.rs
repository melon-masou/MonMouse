mod routing;
pub use routing::*;

use std::{
    fmt::Debug,
    sync::mpsc::{
        channel, sync_channel, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError,
    },
    time::{Duration, Instant},
};

use monmouse_lib::plugin::{PluginKey, PluginMessage};

use crate::{
    device_type::DeviceType,
    errors::Error,
    input_control::Positioning,
    plugin::{interceptor::RegisteredInterceptor, PluginSubscriber},
    setting::{DeviceSettingItem, ProcessorSettings},
};

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
    inner: Box<(TReq, Result<TRsp>)>,
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
            inner: Box::new((req, Err(Error::MessageInited))),
        }
    }

    pub fn req(&self) -> &TReq {
        &self.inner.0
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

    pub fn take_rsp(self) -> Result<TRsp> {
        self.inner.1
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TimerDueKind {
    InspectDevice,
    PluginStop,
}

#[derive(Debug)]
pub enum Message {
    Exit,
    RestartUI,
    Reinitilization,
    TimerDue(TimerDueKind),
    LockCurMouse(String),
    SwitchCurMouse(String),
    ScanDevices(RoundtripData<(), Vec<GenericDevice>>),
    InspectDevicesStatus(RoundtripData<(), Vec<(String, DeviceStatus)>>),
    ApplyProcessorSetting(RoundtripData<ApplyProcessorSettingsData, ()>),
    ApplyOneDeviceSetting(SendData<DeviceSettingItem>),
    SettingsSaved,

    // Plugin Messages
    StartPlugin {
        key: PluginKey,
        interceptor: Option<Box<RegisteredInterceptor>>,
        subscribers: Vec<PluginSubscriber>,
    },
    InitializePluginShortcuts {
        key: PluginKey,
        bindings: Vec<(String, String)>,
    },
    RegisterPluginShortcuts {
        key: PluginKey,
        bindings: Vec<(String, String)>,
    },
    StopPlugin(PluginKey),
    PluginReactorStopped(PluginKey),
    PluginMessage {
        key: PluginKey,
        message: PluginMessage,
    },
    PluginError {
        text: String,
        alert: bool,
    },
}

#[derive(Debug)]
pub struct ApplyProcessorSettingsData {
    pub user_requested: bool,
    pub proc_settings: ProcessorSettings,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug)]
pub enum ShortcutID {
    CurMouseLock = 1000,
    CurMouseJumpNext = 1001,
    CurMouseSwitch = 1002,
    CurMouseFreeze = 1003,
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

pub trait UINotify: Send + Sync {
    fn notify(&self);
}

pub enum TimerOperation {
    ResetInterval(Duration),
    WakeAt(Instant, TimerDueKind),
}

pub struct TimerOperator {
    op_tx: Sender<TimerOperation>,
    handle: std::thread::JoinHandle<()>,
}

impl TimerOperator {
    pub fn update_interval(&self, dur: Duration) {
        let _ = self.op_tx.send(TimerOperation::ResetInterval(dur));
    }
    pub fn wake_at(&self, deadline: Instant, kind: TimerDueKind) {
        let _ = self.op_tx.send(TimerOperation::WakeAt(deadline, kind));
    }
    pub fn stop(self) {
        drop(self.op_tx);
        let _ = self.handle.join();
    }
}

pub fn timer_spawn(sender: MessageSender, kind: TimerDueKind) -> TimerOperator {
    let (op_tx, op_rx) = channel::<TimerOperation>();

    let handle = std::thread::spawn(move || {
        let mut periodic = None::<(Duration, Instant)>;
        let mut wakeup = None::<(Instant, TimerDueKind)>;
        loop {
            let now = Instant::now();
            if let Some((deadline, kind)) = wakeup {
                if now >= deadline {
                    wakeup = None;
                    sender.send(Message::TimerDue(kind));
                }
            }
            if let Some((interval, deadline)) = &mut periodic {
                if now >= *deadline {
                    *deadline = now + *interval;
                    sender.send(Message::TimerDue(kind));
                }
            }

            let next = periodic
                .map(|(_, deadline)| deadline)
                .into_iter()
                .chain(wakeup.map(|(deadline, _)| deadline))
                .min();
            let operation = match next {
                Some(deadline) => {
                    op_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                }
                None => op_rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match operation {
                Ok(TimerOperation::ResetInterval(interval)) => {
                    periodic = Some((interval, Instant::now() + interval));
                }
                Ok(TimerOperation::WakeAt(deadline, kind)) => wakeup = Some((deadline, kind)),
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    });

    TimerOperator { op_tx, handle }
}
