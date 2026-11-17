use std::{mem::size_of, sync::OnceLock};

use log::{debug, error, trace};
use windows::Win32::{
    Foundation::LPARAM,
    UI::{
        Input::{HRAWINPUT, RAWINPUT},
        WindowsAndMessaging::{
            HHOOK, MSG, MSLLHOOKSTRUCT, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, WM_HOTKEY,
            WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_POWERBROADCAST,
        },
    },
};

use super::{
    constants::RAWINPUT_MSG_INIT_BUF_SIZE,
    service::WinPlatformService,
    winwrap::{
        input::{
            check_mouse_event_is_absolute, get_rawinput_data, rawinput_to_string, set_windows_hook,
            unset_windows_hook, HookWrap, MouseLowLevelHook,
        },
        window::{poll_window_messages, widen_tick},
    },
};
use crate::{
    errors::Result,
    input_control::{DeviceId, MousePos, Positioning},
    message::{Message, MessageSender},
    processor::Processor,
};

struct WinHook {
    mouse_ll_hook: Option<HHOOK>,
}

impl WinHook {
    fn new() -> Self {
        WinHook {
            mouse_ll_hook: None,
        }
    }

    fn register(&mut self) -> Result<()> {
        self.mouse_ll_hook = Some(set_windows_hook(HookWrap::mouse_ll::<WinHook>())?);
        Ok(())
    }

    fn unregister(&mut self) -> Result<()> {
        if let Some(hook) = self.mouse_ll_hook.take() {
            unset_windows_hook(hook)?;
        }
        Ok(())
    }

    fn set_ev_sender(sender: MessageSender) {
        G_HOOK_EV_SENDER.set(sender).unwrap();
    }
}

static G_HOOK_EV_SENDER: OnceLock<MessageSender> = OnceLock::new();
impl MouseLowLevelHook for WinHook {
    fn on_mouse_ll(action: u32, e: &mut MSLLHOOKSTRUCT) -> bool {
        let hook_ev_sender = G_HOOK_EV_SENDER.get().unwrap();

        trace!(
            "mousell hook: action={}, pt=({},{})",
            action,
            e.pt.x,
            e.pt.y
        );

        hook_ev_sender.send(Message::MousePosition {
            position: MousePos::from(e.pt.x, e.pt.y),
            tick: widen_tick(e.time),
        });
        true
    }
}

pub(super) struct WinPoller {
    hook: WinHook,
    raw_input_buf: Vec<RAWINPUT>,
    ui_tx: MessageSender,
}

impl WinPoller {
    pub(super) fn new(ui_tx: MessageSender, input_tx: MessageSender) -> Self {
        WinHook::set_ev_sender(input_tx);
        Self {
            hook: WinHook::new(),
            raw_input_buf: vec![
                RAWINPUT::default();
                (RAWINPUT_MSG_INIT_BUF_SIZE as usize)
                    .div_ceil(size_of::<RAWINPUT>())
            ],
            ui_tx,
        }
    }

    pub(super) fn initialize(&mut self) -> Result<()> {
        self.hook.register()
    }

    pub(super) fn terminate(&mut self) -> Result<()> {
        self.hook.unregister()
    }

    fn on_raw_input(
        &mut self,
        lparam: LPARAM,
        tick: u32,
        processor: &mut Processor,
        platform: &mut WinPlatformService,
    ) {
        let tick = widen_tick(tick);
        if let Err(error) =
            get_rawinput_data(HRAWINPUT(lparam.0 as *mut _), &mut self.raw_input_buf)
        {
            error!("Get rawinput data failed: {}", error);
            return;
        }
        let input = &self.raw_input_buf[0];
        let positioning = match check_mouse_event_is_absolute(input) {
            Some(true) => Positioning::Absolute,
            Some(false) => Positioning::Relative,
            None => Positioning::Unknown,
        };
        trace!(
            "rawinput msg: tick={} msg {}",
            tick,
            rawinput_to_string(input)
        );
        processor.on_device_activity(
            (!input.header.hDevice.is_invalid()).then_some(input.header.hDevice.0 as DeviceId),
            positioning,
            tick,
            platform,
        );
    }

    fn handle_wm_message(
        &mut self,
        message: &MSG,
        processor: &mut Processor,
        platform: &mut WinPlatformService,
    ) {
        match message.message {
            WM_INPUT => self.on_raw_input(message.lParam, message.time, processor, platform),
            WM_INPUT_DEVICE_CHANGE => {
                debug!("Trigger updating devices by WM_INPUT_DEVICE_CHANGE");
                processor.on_devices_changed();
            }
            WM_POWERBROADCAST => match message.wParam.0 as u32 {
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                    debug!("Reinitialize after system resume");
                    processor.reinitialize(platform);
                }
                _ => (),
            },
            WM_HOTKEY => {
                let shortcut = platform
                    .hotkey_mgr
                    .get_callback(message.lParam.0 as u32)
                    .copied();
                processor.on_shortcut(shortcut, platform, &self.ui_tx);
            }
            _ => (),
        }
    }

    fn forward_window_events(processor: &mut Processor, platform: &mut WinPlatformService) {
        if std::mem::take(&mut platform.monitors_changed) {
            processor.on_monitors_changed();
        }
    }

    pub(super) fn poll(
        &mut self,
        max_events: u32,
        processor: &mut Processor,
        platform: &mut WinPlatformService,
    ) -> Result<bool> {
        if !poll_window_messages(max_events, |message| {
            // Dispatch of the previous message can report display changes.
            Self::forward_window_events(processor, platform);
            self.handle_wm_message(message, processor, platform);
        }) {
            return Ok(false);
        }
        Self::forward_window_events(processor, platform);
        processor.resolve_pending_updating_task(platform);
        Ok(true)
    }
}
