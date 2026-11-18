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

#[derive(Clone)]
pub struct MessageSender {
    tx: Sender<Message>,
    notify: Option<Arc<dyn UINotify>>,
}

impl MessageSender {
    pub fn send(&self, message: Message) {
        if self.tx.send(message).is_ok() {
            if let Some(notify) = &self.notify {
                notify.notify();
            }
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
    pub mouse_control: Reactor,
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
        bus,
        mouse_control: mouse,
        ui,
    }
}
