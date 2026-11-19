pub mod backend;
pub mod interceptor;
#[cfg(target_os = "windows")]
pub mod loader;
pub mod manager;

use std::{
    sync::mpsc::{channel, Receiver, Sender},
    thread::{self, JoinHandle},
};

use log::error;
use monmouse_lib::plugin::{
    EventKind, Inbox, InboxError, PluginKey, PluginMessage, PluginTarget, ReactorTask,
};

use crate::message::{Message, MessageBus, MessageSender};

use backend::ReactorContextHost;

#[derive(Clone)]
pub struct PluginRouter {
    pub key: PluginKey,
    pub bus: MessageBus,
    pub reactors: Vec<Sender<PluginMessage>>,
    pub has_interceptor: bool,
}

impl PluginRouter {
    pub fn send(&self, target: PluginTarget, message: PluginMessage) -> bool {
        let destination = match target {
            PluginTarget::Reactor(id) => {
                return self
                    .reactors
                    .get(id)
                    .is_some_and(|sender| sender.send(message).is_ok());
            }
            PluginTarget::Main => &self.bus.ui,
            PluginTarget::Interceptor if self.has_interceptor => &self.bus.mouse_control,
            PluginTarget::Interceptor => return false,
        };
        destination.send(Message::PluginMessage {
            key: self.key,
            message,
        })
    }
}

#[derive(Debug)]
pub struct PluginSubscriber {
    pub events: &'static [EventKind],
    pub reactor: Option<Sender<PluginMessage>>,
}

impl PluginSubscriber {
    pub fn publish(
        &self,
        kind: EventKind,
        key: PluginKey,
        message: impl FnOnce() -> PluginMessage,
        ui: &MessageSender,
    ) {
        if !self.events.contains(&kind) {
            return;
        }
        let message = message();
        if let Some(sender) = &self.reactor {
            let _ = sender.send(message);
        } else {
            ui.send(Message::PluginMessage { key, message });
        }
    }
}

struct HostInbox(Receiver<PluginMessage>);

impl Inbox for HostInbox {
    fn recv(&self) -> Option<PluginMessage> {
        self.0.recv().ok()
    }
    fn recv_timeout(&self, timeout: std::time::Duration) -> Result<PluginMessage, InboxError> {
        use std::sync::mpsc::RecvTimeoutError;
        self.0.recv_timeout(timeout).map_err(|error| match error {
            RecvTimeoutError::Timeout => InboxError::Timeout,
            RecvTimeoutError::Disconnected => InboxError::Disconnected,
        })
    }
}

pub struct PluginRuntime {
    workers: Vec<JoinHandle<()>>,
    key: PluginKey,
    senders: Vec<Sender<PluginMessage>>,
}

impl PluginRuntime {
    pub fn start(
        tasks: Vec<Box<dyn ReactorTask>>,
        context: ReactorContextHost,
        inboxes: Vec<Receiver<PluginMessage>>,
    ) -> Self {
        let workers = tasks
            .into_iter()
            .zip(inboxes)
            .map(|(task, reactor)| {
                let context = context.clone();
                thread::spawn(move || task.run(&HostInbox(reactor), &context))
            })
            .collect();
        Self {
            workers,
            key: context.router.key,
            senders: context.router.reactors,
        }
    }

    pub fn channels(count: usize) -> (Vec<Sender<PluginMessage>>, Vec<Receiver<PluginMessage>>) {
        (0..count).map(|_| channel()).unzip()
    }
}

impl Drop for PluginRuntime {
    fn drop(&mut self) {
        for sender in &self.senders {
            let _ = sender.send(PluginMessage::Stop);
        }
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                error!("Plugin {} reactor panicked", self.key);
            }
        }
    }
}
