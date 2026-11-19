use std::{
    collections::BTreeSet,
    path::Path,
    time::{Duration, Instant},
};

use log::warn;

use monmouse_lib::plugin::{
    EventKind, PluginDefine, PluginKey, PluginMain, PluginMessage, PluginUi,
};

use super::{
    backend::{MainContextHost, ReactorContextHost},
    interceptor::RegisteredInterceptor,
    PluginRuntime, PluginSubscriber,
};
use crate::message::{Message, TimerDueKind, TimerOperator};

const STOP_TIMEOUT: Duration = Duration::from_secs(3);

enum PluginState {
    Disabled,
    Running,
    Stopping,
    StopTimedOut,
}

#[derive(Default)]
pub struct PluginManager {
    pub entries: Vec<PluginEntry>,
}

impl PluginManager {
    pub fn initialize(&mut self, enabled: &BTreeSet<String>) {
        for plugin in &mut self.entries {
            if enabled.contains(plugin.key) {
                plugin.start();
            }
        }
    }

    pub fn stop_all(&mut self, timer: &TimerOperator) {
        for plugin in &mut self.entries {
            plugin.stop_reactors();
        }
        self.schedule_stop_timeout(timer);
    }

    pub fn all_stopped(&self) -> bool {
        self.entries
            .iter()
            .all(|plugin| !plugin.enabled() && !plugin.stopping())
    }

    pub fn check_stop_timeouts(&mut self, timer: &TimerOperator) {
        let now = Instant::now();
        for plugin in &mut self.entries {
            plugin.check_stop_timeout(now);
        }
        self.schedule_stop_timeout(timer);
    }

    pub fn schedule_stop_timeout(&self, timer: &TimerOperator) {
        if let Some(deadline) = self
            .entries
            .iter()
            .filter_map(|plugin| plugin.stop_deadline)
            .min()
        {
            timer.wake_at(deadline, TimerDueKind::PluginStop);
        }
    }

    pub fn entry_mut(&mut self, key: PluginKey) -> Option<&mut PluginEntry> {
        self.entries.iter_mut().find(|plugin| plugin.key == key)
    }
}

pub struct PluginEntry {
    pub key: PluginKey,
    pub name: &'static str,
    pub main: Box<dyn PluginMain>,
    pub tabs: &'static [&'static str],

    state: PluginState,
    runtime: PluginRuntime,
    stop_deadline: Option<Instant>,
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

            state: PluginState::Disabled,
            runtime: PluginRuntime::default(),
            stop_deadline: None,
            subscriptions: define.subscriptions,
            main_context: MainContextHost {
                router: reactor_context.router.clone(),
                config_dir: config_dir.map(Path::to_path_buf),
            },
            reactor_context,
        }
    }

    pub fn enabled(&self) -> bool {
        matches!(self.state, PluginState::Running)
    }

    pub fn stopping(&self) -> bool {
        matches!(self.state, PluginState::Stopping)
    }

    pub fn can_toggle(&self) -> bool {
        match self.state {
            PluginState::Disabled => true,
            PluginState::Running => !self.main.is_saving(),
            PluginState::Stopping | PluginState::StopTimedOut => false,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        match (&self.state, enabled) {
            (PluginState::Disabled, true) => self.start(),
            (PluginState::Stopping, true) => return Err("Plugin is stopping".into()),
            (PluginState::StopTimedOut, true) => return Err("Plugin cannot be enabled".into()),
            (PluginState::Running, false) => {
                if self.main.is_saving() {
                    return Err("Plugin settings are being saved".into());
                }
                self.stop();
            }
            _ => (),
        }
        Ok(())
    }

    fn start(&mut self) {
        if !matches!(self.state, PluginState::Disabled) {
            return;
        }
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
        let interceptor = handler.map(|handler| {
            Box::new(RegisteredInterceptor {
                context: context.clone(),
                handler,
            })
        });
        if interceptor.is_some() || !subscribers.is_empty() {
            context.router.bus.mouse_control.send(Message::StartPlugin {
                key: self.key,
                interceptor,
                subscribers,
            });
        }
        self.runtime = PluginRuntime::start(tasks, context, inboxes);
        self.state = PluginState::Running;
    }

    fn stop(&mut self) {
        self.main_context
            .router
            .bus
            .mouse_control
            .send(Message::StopPlugin(self.key));
        self.stop_reactors();
    }

    fn stop_reactors(&mut self) {
        if !self.enabled() {
            return;
        }
        self.state = PluginState::Stopping;
        self.stop_deadline = Some(Instant::now() + STOP_TIMEOUT);
        self.runtime.request_stop();
        self.try_finish_stop();
    }

    pub fn reactor_stopped(&mut self) {
        if self.enabled() || self.stopping() {
            self.runtime.reactor_stopped();
            self.try_finish_stop();
        }
    }

    fn try_finish_stop(&mut self) {
        if self.stopping() && self.runtime.is_stopped() {
            self.finish_stop(false);
        }
    }

    fn finish_stop(&mut self, timed_out: bool) {
        self.runtime = PluginRuntime::default();
        self.stop_deadline = None;
        self.state = if timed_out {
            PluginState::StopTimedOut
        } else {
            PluginState::Disabled
        };
        self.main_context.router.reactors.clear();
        self.main_context.router.has_interceptor = false;
        if !timed_out {
            self.main.discard_changes();
            self.main.stop();
        }
    }

    fn check_stop_timeout(&mut self, now: Instant) {
        if self.stop_deadline.is_some_and(|deadline| now >= deadline) {
            warn!("Timed out stopping plugin {}", self.key);
            self.finish_stop(true);
        }
    }

    pub fn on_event(&mut self, message: &PluginMessage) {
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
