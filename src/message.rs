use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};

pub enum DeviceStatus {
    Active,
    Idle,
    Disconnected,
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

pub enum UIPendingAction {
    Restart,
    Exit,
}

pub fn setup_reactors() -> (MasterReactor, MouseControlReactor, UIReactor) {
    let (ui_close_tx, ui_close_rx) = signal();
    let (ui_pending_tx, ui_pending_rx) = channel::<UIPendingAction>();

    let master = MasterReactor {
        ui_close_tx,
        ui_pending_tx,
    };
    let mouse_ctrl = MouseControlReactor {};
    let frontend = UIReactor {
        ui_close_rx,
        ui_pending_rx,
    };

    (master, mouse_ctrl, frontend)
}

pub struct MasterReactor {
    ui_close_tx: SignalSender,
    ui_pending_tx: Sender<UIPendingAction>,
}

impl MasterReactor {
    pub fn exit_ui(&self) {
        self.ui_pending_tx.send(UIPendingAction::Exit).unwrap();
        self.ui_close_tx.send()
    }
    pub fn restart_ui(&self) {
        self.ui_pending_tx.send(UIPendingAction::Restart).unwrap();
    }
    pub fn close_ui(&self) {
        self.ui_pending_tx.send(UIPendingAction::Exit).unwrap();
    }
}

pub struct MouseControlReactor {}

pub struct UIReactor {
    ui_close_rx: SignalReceiver,
    ui_pending_rx: Receiver<UIPendingAction>,
}

impl UIReactor {
    pub fn check_close(&self) -> bool {
        self.ui_close_rx.check().is_some()
    }
    pub fn recv_pending_msg(&self, wait_one: bool) -> UIPendingAction {
        if wait_one {
            match self.ui_pending_rx.recv() {
                Ok(UIPendingAction::Exit) => return UIPendingAction::Exit,
                Ok(UIPendingAction::Restart) => (),
                Err(_) => return UIPendingAction::Exit,
            }
        }
        loop {
            match self.ui_pending_rx.try_recv() {
                Ok(UIPendingAction::Exit) => return UIPendingAction::Exit,
                Ok(UIPendingAction::Restart) => (),
                Err(_) => break,
            }
        }
        UIPendingAction::Restart
    }
}
