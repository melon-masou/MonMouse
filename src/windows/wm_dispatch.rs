use log::{debug, error, trace};
use windows::Win32::{
    Foundation::LPARAM,
    UI::{
        Input::{HRAWINPUT, RAWINPUT},
        WindowsAndMessaging::{
            MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, WM_DISPLAYCHANGE, WM_DPICHANGED,
            WM_HOTKEY, WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_POWERBROADCAST,
        },
    },
};

use super::{
    service::WinPlatformService,
    winwrap::{
        input::{check_mouse_event_is_absolute, get_rawinput_data, rawinput_to_string},
        window::widen_tick,
    },
};
use crate::{
    input_control::{DeviceId, Positioning},
    message::MessageBus,
    processor::{effects::EffectsIntent, platform::PlatformService, Processor},
};

pub struct WMDispatch;

impl WMDispatch {
    pub fn dispatch(
        processor: &mut Processor,
        platform: &mut WinPlatformService,
        effects: &mut EffectsIntent,
        raw_input_buf: &mut Vec<RAWINPUT>,
        message: &MSG,
        bus: &MessageBus,
    ) {
        match message.message {
            WM_INPUT => Self::on_raw_input(
                processor,
                effects,
                raw_input_buf,
                message.lParam,
                message.time,
            ),
            WM_INPUT_DEVICE_CHANGE => {
                debug!("Trigger updating devices by WM_INPUT_DEVICE_CHANGE");
                effects.refresh_devices = true;
            }
            WM_POWERBROADCAST => match message.wParam.0 as u32 {
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                    debug!("Reinitialize after system resume");
                    if let Err(error) = platform.register_raw_devices() {
                        error!("Reinit register raw devices failed: {}", error);
                    }
                }
                _ => (),
            },
            WM_HOTKEY => {
                let shortcut = platform
                    .hotkey_mgr
                    .get_callback(message.lParam.0 as u32)
                    .copied();
                if let Some(ui_message) =
                    processor.on_shortcut(shortcut, widen_tick(message.time), effects)
                {
                    bus.ui.send(ui_message);
                }
            }
            _ => (),
        }
    }

    pub fn dispatch_overlay_window_wm(effects: &mut EffectsIntent, message: u32) -> bool {
        match message {
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                debug!("Trigger updating monitors by WM {}", message);
                effects.refresh_monitors = true;
            }
            _ => (),
        }
        true
    }

    fn on_raw_input(
        processor: &mut Processor,
        effects: &mut EffectsIntent,
        raw_input_buf: &mut Vec<RAWINPUT>,
        lparam: LPARAM,
        tick: u32,
    ) {
        let tick = widen_tick(tick);
        if let Err(error) = get_rawinput_data(HRAWINPUT(lparam.0 as *mut _), raw_input_buf) {
            error!("Get rawinput data failed: {}", error);
            return;
        }
        let input = &raw_input_buf[0];
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
        let source =
            (!input.header.hDevice.is_invalid()).then_some(input.header.hDevice.0 as DeviceId);
        processor.on_device_activity(source, positioning, tick, effects);
    }
}
