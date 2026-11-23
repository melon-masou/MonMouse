use super::CustomMessage;
use crate::input::{InputInjection, MonitorAreasList};
use std::{any::Any, fmt, time::Duration};

impl dyn CustomMessage {
    pub fn downcast_ref<T: CustomMessage>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }

    pub fn downcast<T: CustomMessage>(self: Box<Self>) -> Option<Box<T>> {
        (self as Box<dyn Any + Send>).downcast().ok()
    }
}

impl fmt::Debug for dyn CustomMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CustomMessage")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    MonitorChange,
}

// A message destination within the calling plugin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginTarget {
    Main,
    Reactor(usize),
    Interceptor,
}

// IDs are stable within a plugin; the host namespaces them with the plugin key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendTarget {
    pub id: String,
    pub name: String,
    pub receiver: PluginTarget,
}

#[derive(Debug)]
pub enum PluginMessage {
    MonitorChange(MonitorAreasList),
    Custom(Box<dyn CustomMessage>),
    Shortcut(String),
    ShortcutRegistrationResult(Result<(), String>),
    Stop,
    SendInput {
        target: String,
        inputs: Vec<InputInjection>,
    },
}

impl PluginMessage {
    pub fn custom(message: impl CustomMessage) -> Self {
        Self::Custom(Box::new(message))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxError {
    Timeout,
    Disconnected,
}

pub trait Inbox {
    fn recv(&self) -> Option<PluginMessage>;
    fn recv_timeout(&self, timeout: Duration) -> Result<PluginMessage, InboxError>;
}
