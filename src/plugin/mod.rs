pub mod backend;
pub mod interceptor;
#[cfg(target_os = "windows")]
pub mod loader;
#[cfg(target_os = "windows")]
pub mod manager;

use std::{
    sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

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
    fn recv_timeout(&self, timeout: Duration) -> Result<PluginMessage, InboxError> {
        self.0.recv_timeout(timeout).map_err(|error| match error {
            RecvTimeoutError::Timeout => InboxError::Timeout,
            RecvTimeoutError::Disconnected => InboxError::Disconnected,
        })
    }
}

#[derive(Default)]
pub struct PluginRuntime {
    running_reactors: usize,
    senders: Vec<Sender<PluginMessage>>,
}

impl PluginRuntime {
    pub fn start(
        tasks: Vec<Box<dyn ReactorTask>>,
        context: ReactorContextHost,
        inboxes: Vec<Receiver<PluginMessage>>,
    ) -> Self {
        let running_reactors = tasks.len();
        let key = context.router.key;
        for (task, reactor) in tasks.into_iter().zip(inboxes) {
            let context = context.clone();
            let ui = context.router.bus.ui.clone();
            thread::spawn(move || {
                task.run(&HostInbox(reactor), &context);
                drop(context);
                ui.send(Message::PluginReactorStopped(key));
            });
        }
        Self {
            running_reactors,
            senders: context.router.reactors,
        }
    }

    pub fn channels(count: usize) -> (Vec<Sender<PluginMessage>>, Vec<Receiver<PluginMessage>>) {
        (0..count).map(|_| channel()).unzip()
    }

    pub fn request_stop(&mut self) {
        for sender in self.senders.drain(..) {
            let _ = sender.send(PluginMessage::Stop);
        }
    }

    fn reactor_stopped(&mut self) {
        self.running_reactors -= 1;
    }

    fn is_stopped(&self) -> bool {
        self.running_reactors == 0
    }
}
