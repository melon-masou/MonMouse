use std::{collections::BTreeSet, path::Path};

use monmouse_lib::plugin::{
    EventKind, MainContext, PluginDefine, PluginKey, PluginMain, PluginMessage, PluginUi,
};

use super::{
    backend::{MainContextHost, ReactorContextHost},
    interceptor::RegisteredInterceptor,
    PluginRuntime, PluginSubscriber,
};
use crate::message::Message;

#[derive(Default)]
pub struct PluginManager {
    pub entries: Vec<PluginEntry>,
}

impl PluginManager {
    pub fn initialize(&mut self, enabled: &BTreeSet<String>) {
        for plugin in &mut self.entries {
            if enabled.contains(plugin.key) && !plugin.enabled() {
                plugin.start();
            }
        }
    }

    pub fn set_enabled(&mut self, key: PluginKey, enabled: bool) -> Result<(), String> {
        let Some(plugin) = self.entries.iter_mut().find(|plugin| plugin.key == key) else {
            return Ok(());
        };
        if enabled && !plugin.enabled() {
            plugin.start();
        }
        if !enabled {
            plugin.stop()?;
        }
        Ok(())
    }

    pub fn on_event(&mut self, key: PluginKey, message: &PluginMessage) {
        if let Some(plugin) = self.entries.iter_mut().find(|plugin| plugin.key == key) {
            plugin.on_event(message);
        }
    }
}

pub struct PluginEntry {
    pub key: PluginKey,
    pub name: &'static str,
    pub main: Box<dyn PluginMain>,
    pub tabs: &'static [&'static str],

    runtime: Option<PluginRuntime>,
    subscriptions: &'static [EventKind],
    main_context: MainContextHost,
    reactor_context: ReactorContextHost,
}

impl PluginEntry {
    pub fn new(
        define: PluginDefine,
        config_dir: Option<&Path>,
        reactor_context: ReactorContextHost,
    ) -> Self {
        Self {
            key: define.key,
            name: define.name,
            main: define.main,
            tabs: define.tabs,

            runtime: None,
            subscriptions: define.subscriptions,
            main_context: MainContextHost {
                router: reactor_context.router.clone(),
                config_dir: config_dir.map(Path::to_path_buf),
            },
            reactor_context,
        }
    }

    pub fn enabled(&self) -> bool {
        self.runtime.is_some()
    }

    fn start(&mut self) {
        let tasks = self.main.init(&self.main_context);
        let handler = self.main.interceptor();
        let (reactors, inboxes) = PluginRuntime::channels(tasks.len());
        let mut subscribers = Vec::new();
        if !self.subscriptions.is_empty() {
            subscribers.push(PluginSubscriber {
                events: self.subscriptions,
                reactor: None,
            });
        }
        for (task, sender) in tasks.iter().zip(&reactors) {
            let events = task.subscriptions();
            if !events.is_empty() {
                subscribers.push(PluginSubscriber {
                    events,
                    reactor: Some(sender.clone()),
                });
            }
        }
        self.main_context.router.reactors = reactors;
        self.main_context.router.has_interceptor = handler.is_some();
        let mut context = self.reactor_context.clone();
        context.router = self.main_context.router.clone();
        if let Some(handler) = handler {
            context
                .router
                .bus
                .mouse_control
                .send(Message::SetPluginInterceptor {
                    key: self.key,
                    interceptor: Some(Box::new(RegisteredInterceptor {
                        context: context.clone(),
                        handler,
                    })),
                });
        }
        if !subscribers.is_empty() {
            context
                .router
                .bus
                .mouse_control
                .send(Message::SetPluginSubscribers {
                    key: self.key,
                    subscribers: Some(subscribers),
                });
        }
        self.runtime = Some(PluginRuntime::start(tasks, context, inboxes));
    }

    fn stop(&mut self) -> Result<(), String> {
        let Some(runtime) = self.runtime.take() else {
            return Ok(());
        };
        let control = &self.main_context.router.bus.mouse_control;
        if self.main_context.router.has_interceptor {
            control.send(Message::SetPluginInterceptor {
                key: self.key,
                interceptor: None,
            });
        }
        control.send(Message::SetPluginSubscribers {
            key: self.key,
            subscribers: None,
        });
        self.main_context.router.reactors.clear();
        self.main_context.router.has_interceptor = false;
        drop(runtime);
        self.main_context.register_shortcuts(Vec::new(), false)
    }

    fn on_event(&mut self, message: &PluginMessage) {
        if self.enabled() {
            self.main.on_event(message, &self.main_context);
        }
    }

    pub fn ui(&mut self, tab: usize, ui: &mut PluginUi<'_>) {
        if self.enabled() {
            self.main.ui(tab, ui, &self.main_context);
        }
    }
}
