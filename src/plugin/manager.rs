use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use log::warn;

use monmouse_lib::plugin::{
    EventKind, PluginDefine, PluginKey, PluginMain, PluginMessage, PluginUi,
};

use super::{
    backend::{MainContextHost, ReactorContextHost},
    interceptor::RegisteredInterceptor,
    loader, PluginRouter, PluginRuntime, PluginSubscriber,
};
use crate::message::{Message, MessageBus, TimerDueKind, TimerOperator};

const STOP_TIMEOUT: Duration = Duration::from_secs(3);

enum PluginState {
    Disabled,
    Running,
    Stopping,
    StopTimedOut,
}

pub struct PluginManager {
    pub entries: Vec<PluginEntry>,
    config_dir: Option<PathBuf>,
    bus: MessageBus,
}

impl PluginManager {
    pub fn new(bus: MessageBus) -> Self {
        Self {
            entries: Vec::new(),
            config_dir: None,
            bus,
        }
    }

    pub fn discover(&mut self, config_dir: Option<&Path>) -> io::Result<()> {
        self.config_dir = config_dir.map(Path::to_path_buf);
        self.entries = loader::discover()?
            .into_iter()
            .map(|(key, path)| PluginEntry {
                key,
                path,
                plugin: None,
            })
            .collect();
        Ok(())
    }

    pub fn initialize(&mut self, enabled: &BTreeSet<String>) {
        for entry in &mut self.entries {
            if enabled.contains(&entry.key) {
                if let Err(text) = entry.set_enabled(true, self.config_dir.as_deref(), &self.bus) {
                    self.bus.ui.send(Message::PluginError { text, alert: true });
                }
            }
        }
    }

    pub fn set_enabled(&mut self, key: &str, enabled: bool) -> Result<(), String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.key == key)
            .ok_or_else(|| format!("Unknown plugin {key}"))?;
        entry.set_enabled(enabled, self.config_dir.as_deref(), &self.bus)
    }

    pub fn stop_all(&mut self, timer: &TimerOperator) {
        for plugin in self.entries.iter_mut().filter_map(PluginEntry::plugin_mut) {
            plugin.stop_reactors();
        }
        self.schedule_stop_timeout(timer);
    }

    pub fn all_stopped(&self) -> bool {
        self.entries
            .iter()
            .all(|entry| !entry.enabled() && !entry.stopping())
    }

    pub fn check_stop_timeouts(&mut self, timer: &TimerOperator) {
        let now = Instant::now();
        for plugin in self.entries.iter_mut().filter_map(PluginEntry::plugin_mut) {
            plugin.check_stop_timeout(now);
        }
        self.schedule_stop_timeout(timer);
    }

    pub fn schedule_stop_timeout(&self, timer: &TimerOperator) {
        if let Some(deadline) = self
            .entries
            .iter()
            .filter_map(PluginEntry::stop_deadline)
            .min()
        {
            timer.wake_at(deadline, TimerDueKind::PluginStop);
        }
    }

    pub fn plugin_mut(&mut self, key: PluginKey) -> Option<&mut Plugin> {
        self.entries
            .iter_mut()
            .find(|entry| entry.key == key)?
            .plugin_mut()
    }
}

pub struct PluginEntry {
    key: String,
    path: PathBuf,
    plugin: Option<Plugin>,
}

impl PluginEntry {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn name(&self) -> &str {
        self.plugin
            .as_ref()
            .map_or(self.key.as_str(), |plugin| plugin.name)
    }

    fn loaded(&self) -> bool {
        self.plugin.is_some()
    }

    pub fn enabled(&self) -> bool {
        self.plugin.as_ref().is_some_and(Plugin::enabled)
    }

    pub fn stopping(&self) -> bool {
        self.plugin.as_ref().is_some_and(Plugin::stopping)
    }

    pub fn can_toggle(&self) -> bool {
        self.plugin.as_ref().is_none_or(Plugin::can_toggle)
    }

    pub fn has_changes(&self) -> bool {
        self.plugin
            .as_ref()
            .is_some_and(|plugin| plugin.enabled() && plugin.main.has_changes())
    }

    pub fn discard_changes(&mut self) {
        if let Some(plugin) = self.plugin_mut() {
            if plugin.main.has_changes() {
                plugin.main.discard_changes();
            }
        }
    }

    pub fn plugin_mut(&mut self) -> Option<&mut Plugin> {
        self.plugin.as_mut()
    }

    fn stop_deadline(&self) -> Option<Instant> {
        self.plugin.as_ref()?.stop_deadline
    }

    fn set_enabled(
        &mut self,
        enabled: bool,
        config_dir: Option<&Path>,
        bus: &MessageBus,
    ) -> Result<(), String> {
        if enabled && !self.loaded() {
            let define = loader::load(&self.path, &self.key)?;
            self.plugin = Some(Plugin::new(define, config_dir, bus));
        }
        if let Some(plugin) = &mut self.plugin {
            plugin.set_enabled(enabled)?;
        }
        Ok(())
    }
}

pub struct Plugin {
    pub key: PluginKey,
    pub name: &'static str,
    main: Box<dyn PluginMain>,
    pub tabs: &'static [&'static str],

    state: PluginState,
    runtime: PluginRuntime,
    stop_deadline: Option<Instant>,
    subscriptions: &'static [EventKind],
    main_context: MainContextHost,
    reactor_context: ReactorContextHost,
}

impl Plugin {
    fn new(define: PluginDefine, config_dir: Option<&Path>, bus: &MessageBus) -> Self {
        let reactor_context = ReactorContextHost {
            router: PluginRouter {
                key: define.key,
                bus: bus.clone(),
                reactors: Vec::new(),
                has_interceptor: false,
            },
            inject_input: crate::windows::winwrap::inject::inject_input,
        };
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

    fn enabled(&self) -> bool {
        matches!(self.state, PluginState::Running)
    }

    fn stopping(&self) -> bool {
        matches!(self.state, PluginState::Stopping)
    }

    fn can_toggle(&self) -> bool {
        match self.state {
            PluginState::Disabled => true,
            PluginState::Running => !self.main.is_saving(),
            PluginState::Stopping | PluginState::StopTimedOut => false,
        }
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
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
