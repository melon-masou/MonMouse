use crate::input::SysMouseEvent;
use std::any::Any;

mod context;
pub use context::*;
mod reactor;
pub use reactor::*;
mod ui;
pub use ui::*;
mod ui_settings;
pub use ui_settings::SettingsPage;

// Plugins should implement this entry point to return their definition.
// MonMouse looks up this exact symbol in the DLL and calls it to register plugins.
//
// #[no_mangle]
// pub fn register_monmouse_plugin_v1() -> PluginDefine
pub type RegisterPlugin = fn() -> PluginDefine;
pub const REGISTER_PLUGIN_SYMBOL: &[u8] = b"register_monmouse_plugin_v1\0";

pub type PluginKey = &'static str;

// MonMouse reads this definition when it loads the DLL, before enabling the plugin.
pub struct PluginDefine {
    // Use a nonempty key containing only a-z, 0-9, and underscores.
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
// MonMouse starts each reactor on its own thread when the plugin is enabled.
pub trait ReactorTask: Send {
    // Selects which host events are sent to the reactor's inbox.
    fn subscriptions(&self) -> &'static [EventKind] {
        &[]
    }
    // Return when the inbox receives `PluginMessage::Stop` so the host can join the thread.
    fn run(self: Box<Self>, inbox: &dyn Inbox, ctx: &dyn ReactorContext);
}

// Implement this trait on the object that owns the plugin's main-thread state.
pub trait PluginMain {
    // Prepare reactor tasks on the UI thread when the plugin is enabled.
    // The task index is used by `PluginTarget::Reactor`.
    fn init(&mut self, _ctx: &dyn MainContext) -> Vec<Box<dyn ReactorTask>> {
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
// Callbacks must not pump Windows messages.
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
