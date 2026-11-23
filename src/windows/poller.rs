use std::{cell::Cell, mem::size_of, ptr};

use log::{debug, error, trace};
use monmouse_lib::{
    input::{InputInjection, MONMOUSE_INPUT_TAG},
    plugin::PluginMessage,
};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::{
        Input::RAWINPUT,
        WindowsAndMessaging::{HHOOK, MSG, MSLLHOOKSTRUCT, WM_HOTKEY},
    },
};

use super::{
    constants::{RAWINPUT_MSG_INIT_BUF_SIZE, SUBCLASS_UID},
    service::WinPlatformService,
    winwrap::{
        input::{
            decode_mouse_event, set_windows_hook, unset_windows_hook, HookWrap, MouseLowLevelHook,
        },
        window::{
            get_cur_tick, poll_window_messages, set_subclass, wait_window_messages, widen_tick,
            SubclassHandler,
        },
    },
    wm_dispatch::WMDispatch,
};
use crate::{
    errors::Result,
    message::{Message, Reactor},
    processor::{
        effects::EffectsIntent,
        message_dispatch::{DispatchResult, MessageDispatch},
        platform::PlatformService,
        shortcuts::ShortcutAction,
        Processor,
    },
    setting::ProcessorSettings,
};

const POLL_MSGS: u32 = 20;
const POLL_TIMEOUT: u32 = 20;
const STALE_MOUSE_EVENT_MS: u64 = POLL_TIMEOUT as u64 * 3;

thread_local! {
    static HOOK_INPUT: Cell<*mut HookInput> = const { Cell::new(ptr::null_mut()) };
}

// Callback effects stay separate from MessageDispatch and are merged only
// before resolution. The fixed allocation also holds the callback's Processor.
struct HookInput {
    processor: Processor,
    effects: EffectsIntent,
}

pub struct Poller {
    hook_input: Box<HookInput>,
    platform: WinPlatformService,
    reactor: Reactor,
    message_dispatch: MessageDispatch,

    mouse_ll_hook: Option<HHOOK>,
    raw_input_buf: Vec<RAWINPUT>,
}

impl Poller {
    pub fn new(headless: bool, reactor: Reactor) -> Self {
        Self {
            hook_input: Box::new(HookInput {
                processor: Processor::new(headless),
                effects: EffectsIntent::default(),
            }),
            platform: WinPlatformService::new(),
            reactor,
            message_dispatch: MessageDispatch::default(),
            mouse_ll_hook: None,
            raw_input_buf: vec![
                RAWINPUT::default();
                (RAWINPUT_MSG_INIT_BUF_SIZE as usize)
                    .div_ceil(size_of::<RAWINPUT>())
            ],
        }
    }

    pub fn initialize_settings(&mut self, settings: &ProcessorSettings) -> Result<()> {
        self.message_dispatch.apply_settings(
            Self::processor_mut(&mut self.hook_input),
            &mut self.platform,
            settings,
            false,
        )
    }

    fn processor_mut(hook_input: &mut Box<HookInput>) -> &mut Processor {
        // Preserve the registered callback pointer when borrowing its fixed allocation.
        let pointer = &raw mut **hook_input;
        unsafe { &mut (*pointer).processor }
    }
    fn merge_hook_callback_effects(&mut self) {
        let pointer = &raw mut *self.hook_input;
        let effects = unsafe { &mut (*pointer).effects };
        self.message_dispatch.effects.merge(*effects);
        effects.clear();
    }

    fn register_callbacks(&mut self) -> Result<()> {
        HOOK_INPUT.with(|current| {
            assert!(
                current.get().is_null(),
                "A mouse hook is already registered on this thread"
            )
        });
        // Callbacks use only their own fixed resources, never dispatcher state.
        // No resource borrow spans native dispatch or output calls.
        let pointer = &raw mut *self.hook_input;
        unsafe {
            set_subclass::<Self>(
                self.platform.hwnd,
                SUBCLASS_UID,
                Some(&raw mut (*pointer).effects),
            )?
        };
        HOOK_INPUT.with(|current| current.set(&raw mut *self.hook_input));
        self.mouse_ll_hook = match set_windows_hook(HookWrap::mouse_ll::<Self>()) {
            Ok(hook) => Some(hook),
            Err(error) => {
                self.unregister_callbacks()?;
                return Err(error);
            }
        };
        Ok(())
    }
    fn unregister_callbacks(&mut self) -> Result<()> {
        if let Some(hook) = self.mouse_ll_hook {
            unset_windows_hook(hook)?;
            self.mouse_ll_hook = None;
        }
        if let Err(e) = unsafe { set_subclass::<Self>(self.platform.hwnd, SUBCLASS_UID, None) } {
            error!("destroy subclass callback failed: {}", e)
        }
        HOOK_INPUT.with(|current| current.set(ptr::null_mut()));
        Ok(())
    }

    fn resolve_effects(&mut self) {
        self.merge_hook_callback_effects();

        self.message_dispatch.resolve_effects(
            Self::processor_mut(&mut self.hook_input),
            &mut self.platform,
            &self.reactor.bus.ui,
        );
        let effects = &mut self.message_dispatch.effects;
        let cursor_drawing = {
            if effects.redraw_cursors {
                effects.redraw_cursors = false;
                let processor = Self::processor_mut(&mut self.hook_input);
                Some(processor.cursor_drawing())
            } else {
                None
            }
        };

        let platform = &mut self.platform;
        if let Some(relocation) = effects.relocation.take() {
            let position = relocation.position;
            let _ = platform.set_cursor_pos(position);
            debug!("Reset cursor to ({},{})", position.x, position.y);
        }
        if effects.refresh_overlay {
            if let Err(error) = platform.refresh_overlay_window() {
                error!("refresh overlay window failed: {}", error);
            }
            effects.refresh_overlay = false;
        }
        if let Some(drawing) = cursor_drawing {
            platform.draw_cursors(drawing);
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.platform.setup_window()?;
        if let Err(error) = self.platform.register_raw_devices() {
            error!("Register raw devices failed: {}", error);
            return Err(error);
        }

        if let Err(error) = self.message_dispatch.refresh_monitors(
            Self::processor_mut(&mut self.hook_input),
            &mut self.platform,
        ) {
            error!("Init monitors info failed: {}", error);
            return Err(error);
        }

        self.register_callbacks()?;
        Ok(())
    }

    pub fn terminate(&mut self) -> Result<()> {
        self.unregister_callbacks()
    }

    fn window_event(&mut self, message: &MSG) {
        if message.message == WM_HOTKEY {
            self.on_shortcut(message);
            return;
        }
        let processor = Self::processor_mut(&mut self.hook_input);
        WMDispatch::dispatch(
            processor,
            &mut self.platform,
            &mut self.message_dispatch.effects,
            &mut self.raw_input_buf,
            message,
        );
    }

    fn on_shortcut(&mut self, message: &MSG) {
        match self
            .message_dispatch
            .shortcuts
            .action(message.wParam.0 as i32)
        {
            Some(ShortcutAction::Main(id)) => {
                if let Some(message) = self.hook_input.processor.on_shortcut(
                    Some(*id),
                    widen_tick(message.time),
                    &mut self.message_dispatch.effects,
                ) {
                    self.reactor.bus.ui.send(message);
                }
            }
            Some(ShortcutAction::Plugin(key, action)) => {
                self.reactor.bus.ui.send(Message::PluginMessage {
                    key,
                    message: PluginMessage::Shortcut(action.clone()),
                });
            }
            Some(ShortcutAction::Mapping(mapping)) => {
                let result = if let Some(target) = &mapping.send_target {
                    if self.hook_input.processor.interceptors.on_mapping(
                        &target.plugin,
                        &target.callback,
                        &mapping.to,
                    ) {
                        Ok(())
                    } else {
                        Err(format!(
                            "Plugin mapping handler unavailable: {}",
                            target.plugin
                        ))
                    }
                } else {
                    monmouse_lib::keyboard::parse_chord(&mapping.to, true).and_then(|buttons| {
                        super::winwrap::inject::inject_input(
                            InputInjection::Shortcut { buttons },
                            MONMOUSE_INPUT_TAG,
                        )
                        .map_err(|error| error.to_string())
                    })
                };
                if let Err(error) = result {
                    error!("Could not send mapped shortcut: {error}");
                }
            }
            None => (),
        }
    }

    pub fn poll(&mut self) -> bool {
        // poll windows message
        wait_window_messages(POLL_TIMEOUT);
        if !poll_window_messages(POLL_MSGS, |message| self.window_event(message)) {
            return false;
        }

        self.resolve_effects();

        let mut running = true;
        while let Some(message) = self.reactor.try_recv() {
            let result = self.message_dispatch.dispatch(
                Self::processor_mut(&mut self.hook_input),
                &mut self.platform,
                message,
                &self.reactor.bus,
            );
            match result {
                DispatchResult::Continue => (),
                DispatchResult::Exit => {
                    running = false;
                    break;
                }
            }
        }

        self.resolve_effects();

        running
    }
}

impl MouseLowLevelHook for Poller {
    fn on_mouse_ll(action: u32, event: &mut MSLLHOOKSTRUCT) -> bool {
        let event = decode_mouse_event(action, event);
        trace!("mousell hook: {:?}", event);
        let tick = widen_tick(event.time);
        if get_cur_tick().saturating_sub(tick) > STALE_MOUSE_EVENT_MS {
            return true;
        }

        // WH_MOUSE_LL runs on the installing thread. The context's SendInput
        // backend queues input without dispatching another hook on this stack.
        HOOK_INPUT.with(|current| unsafe {
            let hook = &mut *current.get();
            if hook.processor.interceptors.blocks_mouse_event(&event) {
                return false;
            }
            if event.injected {
                hook.processor
                    .observe_injected_cursor(event.position, tick, &mut hook.effects);
                if event.extra_info == MONMOUSE_INPUT_TAG {
                    return true;
                }
            }
            if let Some(matched) = hook.processor.match_mouse_position(event.position, tick) {
                return hook
                    .processor
                    .on_mouse_event(matched, event.action, &mut hook.effects);
            }
            true
        })
    }
}

impl SubclassHandler for Poller {
    type Context = EffectsIntent;

    fn subclass_callback(
        context: &mut Self::Context,
        message: u32,
        _wp: WPARAM,
        _lp: LPARAM,
        _class: usize,
    ) -> bool {
        WMDispatch::dispatch_overlay_window_wm(context, message)
    }
}
