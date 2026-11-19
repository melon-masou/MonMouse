use std::any::Any;

use crate::input::SysMouseEvent;

mod context;
pub use context::*;
mod reactor;
pub use reactor::*;
mod ui;
pub use ui::*;
mod ui_settings;
pub use ui_settings::SettingsPage;

// Plugins should implement this entry point to return their definition.
// MonMouse looks up this exact symbol in the DLL and call it to register plugins.
//
// #[no_mangle]
// pub fn register_plugin_v1() -> PluginDefine
pub type RegisterPlugin = fn() -> PluginDefine;

pub type PluginKey = &'static str;

// MonMouse reads this definition when it loads the DLL, before enabling the plugin.
pub struct PluginDefine {
    // Keep this key stable so MonMouse can find the plugin's saved settings.
    pub key: PluginKey,
    // Display name shown in the Plugins selector.
    pub name: &'static str,
    // Handles the plugin's UI and messages on the main thread.
    pub main: Box<dyn PluginMain>,
    // Tab labels in the order passed to `PluginMain::ui`.
    pub tabs: &'static [&'static str],
    // Selects the host events delivered to the plugin's main thread.
    pub subscriptions: &'static [EventKind],
}

// The task can capture startup settings before it moves to the reactor thread.
pub type ReactorTask = dyn FnOnce(Reactor, &dyn ReactorContext) + Send;

// MonMouse starts each reactor on its own thread when the plugin is enabled.
pub struct ReactorDefine {
    // Return when the inbox receives `PluginMessage::Stop` so the host can join the thread.
    pub run: Box<ReactorTask>,
    // Selects which host events are sent to the reactor's inbox.
    pub subscriptions: &'static [EventKind],
}

// Implement this trait on the object that owns the plugin's main-thread state.
pub trait PluginMain {
    // Prepare reactor tasks on the UI thread when the plugin is enabled.
    // The task index is used by `PluginTarget::Reactor`.
    fn init(&mut self, _ctx: &dyn MainContext) -> Vec<ReactorDefine> {
        Vec::new()
    }

    // Create the optional interceptor that runs on the input thread.
    fn interceptor(&mut self) -> Option<PluginInterceptor> {
        None
    }

    // Draws the selected tab. The index is zero for plugins without tab labels.
    fn ui(&mut self, tab: usize, ui: &mut PluginUi<'_>, ctx: &dyn MainContext);

    // Handles custom messages, shortcuts, and subscribed host events on the UI
    // thread, whether or not the plugin's UI is visible.
    fn on_event(&mut self, _event: &PluginMessage, _ctx: &dyn MainContext) {}

    // Return true to ask for confirmation before leaving a tab with unsaved edits.
    fn has_changes(&self) -> bool {
        false
    }

    // Drop unsaved edits after the user confirms discard.
    fn discard_changes(&mut self) {}
}

pub type InterceptorMessageHandler = dyn FnMut(PluginMessage, &dyn ReactorContext) + Send;
pub type MouseEventFilter = dyn FnMut(&SysMouseEvent, &dyn ReactorContext) -> bool + Send;

// Selects the callbacks the plugin runs on the input thread.
#[derive(Default)]
pub struct PluginInterceptor {
    // Handles messages addressed to this interceptor outside the native input callback.
    pub on_event: Option<Box<InterceptorMessageHandler>>,
    // Runs before mouse device matching. All filters run in plugin key order.
    // The event is blocked if any filter returns true.
    pub block_mouse_event: Option<Box<MouseEventFilter>>,
}

// Implement this for plugin-defined messages carried by `PluginMessage::Custom`.
pub trait CustomMessage: Any + Send {}
