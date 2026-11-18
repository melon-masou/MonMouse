use std::{cell::Cell, mem::size_of, ptr};

use log::{debug, error, trace};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::{
        Input::RAWINPUT,
        WindowsAndMessaging::{HHOOK, MSG, MSLLHOOKSTRUCT},
    },
};

use super::{
    constants::{RAWINPUT_MSG_INIT_BUF_SIZE, SUBCLASS_UID},
    service::WinPlatformService,
    winwrap::{
        input::{set_windows_hook, unset_windows_hook, HookWrap, MouseLowLevelHook},
        window::{
            get_cur_tick, poll_window_messages, set_subclass, wait_window_messages, widen_tick,
            SubclassHandler,
        },
    },
    wm_dispatch::WMDispatch,
};
use crate::{
    errors::Result,
    input_control::MousePos,
    message::{GenericDevice, MouseControlReactor},
    processor::{
        effects::EffectsIntent,
        message_dispatch::{DispatchResult, MessageDispatch},
        platform::PlatformService,
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
    reactor: MouseControlReactor,
    message_dispatch: MessageDispatch,

    mouse_ll_hook: Option<HHOOK>,
    raw_input_buf: Vec<RAWINPUT>,
}

impl Poller {
    pub fn new(headless: bool, reactor: MouseControlReactor) -> Self {
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

    pub fn scan_devices(&mut self) -> Result<Vec<GenericDevice>> {
        self.message_dispatch.scan_devices(
            Self::processor_mut(&mut self.hook_input),
            &mut self.platform,
        )
    }

    pub fn initialize_settings(&mut self, settings: &ProcessorSettings) -> Result<()> {
        self.message_dispatch.apply_settings(
            Self::processor_mut(&mut self.hook_input),
            &mut self.platform,
            settings,
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
        // Both callbacks use only their own fixed resources, never dispatcher state.
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
        match set_windows_hook(HookWrap::mouse_ll::<Self>()) {
            Ok(hook) => {
                self.mouse_ll_hook = Some(hook);
                Ok(())
            }
            Err(error) => {
                self.unregister_callbacks()?;
                Err(error)
            }
        }
    }
    fn unregister_callbacks(&mut self) -> Result<()> {
        if let Some(hook) = self.mouse_ll_hook {
            unset_windows_hook(hook)?;
            self.mouse_ll_hook = None;
        }
        unsafe { set_subclass::<Self>(self.platform.hwnd, SUBCLASS_UID, None)? };
        HOOK_INPUT.with(|current| current.set(ptr::null_mut()));
        Ok(())
    }

    fn resolve_effects(&mut self) {
        self.merge_hook_callback_effects();

        self.message_dispatch.resolve_effects(
            Self::processor_mut(&mut self.hook_input),
            &mut self.platform,
        );
        let effects = &mut self.message_dispatch.effects;
        let inactive_cursors_redraw = {
            if effects.redraw_inactive_cursors {
                effects.redraw_inactive_cursors = false;
                let processor = Self::processor_mut(&mut self.hook_input);
                Some(processor.inactive_cursor_drawing())
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
        if let Some(drawing) = inactive_cursors_redraw {
            platform.draw_cursors(drawing.positions, drawing.show_markers);
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
        self.unregister_callbacks()?;
        Ok(())
    }

    fn window_event(&mut self, message: &MSG) {
        let processor = Self::processor_mut(&mut self.hook_input);
        let reply = WMDispatch::dispatch(
            processor,
            &mut self.platform,
            &mut self.message_dispatch.effects,
            &mut self.raw_input_buf,
            message,
        );
        if let Some(message) = reply {
            self.reactor.ui_tx.send(message);
        }
    }

    pub fn poll(&mut self) -> bool {
        // poll windows message
        wait_window_messages(POLL_TIMEOUT);
        if !poll_window_messages(POLL_MSGS, |message| self.window_event(message)) {
            return false;
        }

        self.resolve_effects();

        // pool messages
        let mut running = true;
        while let Some(message) = self.reactor.mouse_control_rx.try_recv() {
            let result = self.message_dispatch.dispatch(
                Self::processor_mut(&mut self.hook_input),
                &mut self.platform,
                message,
            );
            match result {
                DispatchResult::Continue => (),
                DispatchResult::Exit => {
                    running = false;
                    break;
                }
                DispatchResult::Reply(message) => self.reactor.return_msg(message),
            }
        }

        self.resolve_effects();

        running
    }
}

impl MouseLowLevelHook for Poller {
    fn on_mouse_ll(action: u32, event: &mut MSLLHOOKSTRUCT) -> bool {
        trace!(
            "mousell hook: action={}, pt=({},{})",
            action,
            event.pt.x,
            event.pt.y
        );
        let tick = widen_tick(event.time);
        if get_cur_tick().saturating_sub(tick) > STALE_MOUSE_EVENT_MS {
            return true;
        }
        let position = MousePos::from(event.pt.x, event.pt.y);
        // WH_MOUSE_LL runs synchronously on its installing thread. Input processing
        // here does not pump messages or call the platform.
        HOOK_INPUT.with(|current| unsafe {
            let hook_input = &mut *current.get();
            hook_input
                .processor
                .on_mouse_position(position, tick, &mut hook_input.effects);
        });
        true
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
