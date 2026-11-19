use std::mem::size_of;

use windows::{
    core::HRESULT,
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT},
            Input::{
                GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
                RAWINPUTHEADER, RID_DEVICE_INFO_TYPE, RID_INPUT, RIM_TYPEHID, RIM_TYPEMOUSE,
            },
            WindowsAndMessaging::{
                CallNextHookEx, SetWindowsHookExA, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
                LLKHF_EXTENDED, LLKHF_INJECTED, LLKHF_UP, LLMHF_INJECTED, MSLLHOOKSTRUCT,
                WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_LBUTTONDOWN, WM_LBUTTONUP,
                WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL,
                WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
            },
        },
    },
};

use super::{core_error, get_last_error};
use crate::{
    ce,
    errors::{Error, Result},
    input_control::{Key, MouseAction, MouseButton, MousePos, SysKeyboardEvent, SysMouseEvent},
    keyboard::key_windows::shortcut_str_to_win,
};

const RAWINPUT_MOUSE_FLAGS_ABSOLUTE: u16 = 1;
const HRESULT_SHORTCUT_CONFLICT: HRESULT = HRESULT(0x80070581u32 as i32);

pub fn decode_mouse_event(action: u32, event: &MSLLHOOKSTRUCT) -> SysMouseEvent {
    let button = |button, down| Some(MouseAction::Button { button, down });
    let action = match action {
        WM_MOUSEMOVE => Some(MouseAction::Move),
        WM_LBUTTONDOWN => button(MouseButton::Left, true),
        WM_LBUTTONUP => button(MouseButton::Left, false),
        WM_RBUTTONDOWN => button(MouseButton::Right, true),
        WM_RBUTTONUP => button(MouseButton::Right, false),
        WM_MBUTTONDOWN => button(MouseButton::Middle, true),
        WM_MBUTTONUP => button(MouseButton::Middle, false),
        WM_XBUTTONDOWN | WM_XBUTTONUP => match event.mouseData >> 16 {
            1 => button(MouseButton::X1, action == WM_XBUTTONDOWN),
            2 => button(MouseButton::X2, action == WM_XBUTTONDOWN),
            _ => None,
        },
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => Some(MouseAction::Wheel {
            delta: (event.mouseData >> 16) as i16,
            horizontal: action == WM_MOUSEHWHEEL,
        }),
        _ => None,
    };
    SysMouseEvent {
        position: MousePos::from(event.pt.x, event.pt.y),
        time: event.time,
        action,
        injected: event.flags & LLMHF_INJECTED != 0,
        extra_info: event.dwExtraInfo,
    }
}

pub fn decode_keyboard_event(event: &KBDLLHOOKSTRUCT) -> SysKeyboardEvent {
    SysKeyboardEvent {
        time: event.time,
        key: Key {
            scan: event.scanCode as u16,
            virtual_key: event.vkCode as u16,
            extended: event.flags.contains(LLKHF_EXTENDED),
        },
        down: !event.flags.contains(LLKHF_UP),
        injected: event.flags.contains(LLKHF_INJECTED),
        extra_info: event.dwExtraInfo,
    }
}

pub trait MouseLowLevelHook {
    fn on_mouse_ll(action: u32, e: &mut MSLLHOOKSTRUCT) -> bool;
}

pub trait KeyboardLowLevelHook {
    fn on_keyboard_ll(event: SysKeyboardEvent) -> bool;
}

pub struct HookWrap {
    id: WINDOWS_HOOK_ID,
    f: extern "system" fn(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT,
}
impl HookWrap {
    extern "system" fn mouse_ll_hook<T: MouseLowLevelHook>(
        ncode: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if ncode < 0 {
            return unsafe { CallNextHookEx(None, ncode, wparam, lparam) };
        }
        let call_next = T::on_mouse_ll(wparam.0 as u32, unsafe {
            &mut *(lparam.0 as *mut MSLLHOOKSTRUCT)
        });
        if call_next {
            unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
        } else {
            LRESULT(1)
        }
    }
    pub fn mouse_ll<T: MouseLowLevelHook>() -> HookWrap {
        HookWrap {
            id: WH_MOUSE_LL,
            f: HookWrap::mouse_ll_hook::<T>,
        }
    }

    extern "system" fn keyboard_ll_hook<T: KeyboardLowLevelHook>(
        ncode: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if ncode < 0 {
            return unsafe { CallNextHookEx(None, ncode, wparam, lparam) };
        }
        let call_next = T::on_keyboard_ll(decode_keyboard_event(unsafe {
            &*(lparam.0 as *const KBDLLHOOKSTRUCT)
        }));
        if call_next {
            unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
        } else {
            LRESULT(1)
        }
    }
    pub fn keyboard_ll<T: KeyboardLowLevelHook>() -> HookWrap {
        HookWrap {
            id: WH_KEYBOARD_LL,
            f: HookWrap::keyboard_ll_hook::<T>,
        }
    }
}

pub fn set_windows_hook(hook: HookWrap) -> Result<HHOOK> {
    unsafe {
        ce!(SetWindowsHookExA(
            hook.id,
            Some(hook.f),
            HINSTANCE::default(),
            0
        ))
    }
}

pub fn unset_windows_hook(hook: HHOOK) -> Result<()> {
    unsafe { ce!(UnhookWindowsHookEx(hook)) }
}

pub fn register_rawinput_devices(devs: &[RAWINPUTDEVICE]) -> Result<()> {
    let cbsize = size_of::<RAWINPUTDEVICE>() as u32;
    unsafe { ce!(RegisterRawInputDevices(devs, cbsize)) }
}

pub fn get_rawinput_data(handle: HRAWINPUT, data_buf: &mut Vec<RAWINPUT>) -> Result<()> {
    let mut size: u32 = 0;
    let header_size = size_of::<RAWINPUTHEADER>() as u32;
    let res = unsafe { GetRawInputData(handle, RID_INPUT, None, &mut size, header_size) };
    if res != 0 {
        return Err(get_last_error("GetRawInputData"));
    }

    // RAWINPUT storage preserves alignment and can hold variable-length HID data.
    let count = (size as usize).div_ceil(size_of::<RAWINPUT>());
    if data_buf.len() < count {
        data_buf.resize(count, RAWINPUT::default());
    }
    let res = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            Some(data_buf.as_mut_ptr().cast()),
            &mut size,
            header_size,
        )
    };
    if res == u32::MAX {
        return Err(get_last_error("GetRawInputData"));
    }
    Ok(())
}

pub fn rawinput_to_string(ri: &RAWINPUT) -> String {
    match RID_DEVICE_INFO_TYPE(ri.header.dwType) {
        RIM_TYPEMOUSE => {
            let m = unsafe { &ri.data.mouse };
            format!(
                "{{mouse({}); hdl={:?}, llast=({},{}), flag={:?}, extra={}}}",
                ri.header.dwType,
                ri.header.hDevice.0,
                m.lLastX,
                m.lLastY,
                m.usFlags,
                m.ulExtraInformation
            )
        }
        RIM_TYPEHID => {
            let m = unsafe { &ri.data.hid };
            format!(
                "{{hid({}); hdl={:?}, size={}, count={} }}",
                ri.header.dwType, ri.header.hDevice.0, m.dwSizeHid, m.dwCount
            )
        }
        _ => {
            format!(
                "{{other({}), hdl={:?}}}",
                ri.header.dwType, ri.header.hDevice.0
            )
        }
    }
}

pub fn check_mouse_event_is_absolute(ri: &RAWINPUT) -> Option<bool> {
    match RID_DEVICE_INFO_TYPE(ri.header.dwType) {
        RIM_TYPEMOUSE => unsafe {
            Some((ri.data.mouse.usFlags.0 & RAWINPUT_MOUSE_FLAGS_ABSOLUTE) > 0)
        },
        _ => None,
    }
}

pub fn register_shortcut(hwnd: HWND, id: i32, text: &str) -> Result<()> {
    let Some((modifiers, key)) = shortcut_str_to_win(text) else {
        return Err(Error::InvalidShortcut(text.to_owned()));
    };
    match unsafe { RegisterHotKey(hwnd, id, modifiers | MOD_NOREPEAT, key.0 as u32) } {
        Ok(_) => Ok(()),
        Err(e) => match e.code() {
            HRESULT_SHORTCUT_CONFLICT => Err(Error::ShortcutConflict(text.into())),
            _ => Err(core_error(e, "RegisterHotKey")),
        },
    }
}

pub fn unregister_shortcut(hwnd: HWND, id: i32) -> Result<()> {
    unsafe { ce!(UnregisterHotKey(hwnd, id)) }
}
