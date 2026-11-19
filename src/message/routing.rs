use std::sync::{
    mpsc::{channel, Receiver, Sender, TryRecvError},
    Arc,
};

use super::{Message, UINotify};

#[derive(Clone)]
pub struct MessageBus {
    pub ui: MessageSender,
    pub mouse_control: MessageSender,
}

impl MessageBus {
    pub fn new_reactor(&self) -> (MessageSender, Reactor) {
        let (tx, inbox) = channel();
        let sender = MessageSender { tx, notify: None };
        let reactor = Reactor {
            bus: self.clone(),
            inbox,
        };
        (sender, reactor)
    }
}

#[derive(Clone)]
pub struct MessageSender {
    tx: Sender<Message>,
    notify: Option<Arc<dyn UINotify>>,
}

impl MessageSender {
    pub fn send(&self, message: Message) -> bool {
        if self.tx.send(message).is_ok() {
            if let Some(notify) = &self.notify {
                notify.notify();
            }
            true
        } else {
            false
        }
    }
}

pub struct Reactor {
    pub bus: MessageBus,
    inbox: Receiver<Message>,
}

impl Reactor {
    pub fn try_recv(&self) -> Option<Message> {
        match self.inbox.try_recv() {
            Ok(message) => Some(message),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Message::Exit),
        }
    }

    pub fn recv(&self) -> Message {
        self.inbox.recv().unwrap_or(Message::Exit)
    }
}

pub struct Reactors {
    pub bus: MessageBus,
    pub control: Reactor,
    pub ui: Reactor,
}

pub fn setup_reactors(ui_notify: Option<Arc<dyn UINotify>>) -> Reactors {
    let (ui_tx, ui_rx) = channel();
    let (mouse_tx, mouse_rx) = channel();
    let bus = MessageBus {
        ui: MessageSender {
            tx: ui_tx,
            notify: ui_notify,
        },
        mouse_control: MessageSender {
            tx: mouse_tx,
            notify: None,
        },
    };
    let mouse = Reactor {
        bus: bus.clone(),
        inbox: mouse_rx,
    };
    let ui = Reactor {
        bus: bus.clone(),
        inbox: ui_rx,
    };
    Reactors {
        control: mouse,
        bus,
        ui,
    }
}
