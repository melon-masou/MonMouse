use std::{collections::BTreeMap, mem::size_of};

use windows::{
    core::HRESULT,
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{
                RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT, VIRTUAL_KEY,
            },
            Input::{
                GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
                RAWINPUTHEADER, RID_DEVICE_INFO_TYPE, RID_INPUT, RIM_TYPEHID, RIM_TYPEMOUSE,
            },
            WindowsAndMessaging::{
                CallNextHookEx, SetWindowsHookExA, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT,
                WH_MOUSE_LL, WINDOWS_HOOK_ID,
            },
        },
    },
};

use super::{core_error, get_last_error};
use crate::{
    ce,
    errors::{Error, Result},
};

const RAWINPUT_MOUSE_FLAGS_ABSOLUTE: u16 = 1;
const HRESULT_SHORTCUT_CONFLICT: HRESULT = HRESULT(0x80070581u32 as i32);

pub trait MouseLowLevelHook {
    fn on_mouse_ll(action: u32, e: &mut MSLLHOOKSTRUCT) -> bool;
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

pub fn register_hot_key(
    hwnd: HWND,
    id: i32,
    mut modifiers: HOT_KEY_MODIFIERS,
    key: VIRTUAL_KEY,
    repeat: bool,
) -> Result<u32> {
    let callback_lparam = ((key.0 as u32) << 16) | modifiers.0;
    if !repeat {
        modifiers |= MOD_NOREPEAT;
    }
    match unsafe { RegisterHotKey(hwnd, id, modifiers, key.0 as u32) } {
        Ok(_) => Ok(callback_lparam),
        Err(e) => match e.code() {
            HRESULT_SHORTCUT_CONFLICT => Err(Error::ShortcutConflict(None.into())),
            _ => Err(core_error(e, "RegisterHotKey")),
        },
    }
}

pub fn unregister_hot_key(hwnd: HWND, id: i32) -> Result<()> {
    unsafe { ce!(UnregisterHotKey(hwnd, id)) }
}

pub struct HotKeyManager<T> {
    id_to_lparam: BTreeMap<i32, u32>,
    lparam_to_cb: BTreeMap<u32, T>,
}

impl<T> HotKeyManager<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            id_to_lparam: BTreeMap::new(),
            lparam_to_cb: BTreeMap::new(),
        }
    }

    pub fn register(
        &mut self,
        hwnd: HWND,
        id: i32,
        modifiers: HOT_KEY_MODIFIERS,
        key: VIRTUAL_KEY,
        repeat: bool,
        cb: T,
    ) -> Result<()> {
        if let Some(h) = self.id_to_lparam.get(&id) {
            self.lparam_to_cb.remove(h);
            self.id_to_lparam.remove(&id);
        }
        let _ = unregister_hot_key(hwnd, id);

        let h = register_hot_key(hwnd, id, modifiers, key, repeat)?;
        self.id_to_lparam.insert(id, h);
        self.lparam_to_cb.insert(h, cb);
        Ok(())
    }

    pub fn unregister(&mut self, hwnd: HWND, id: i32) -> Result<()> {
        if let Some(h) = self.id_to_lparam.get(&id) {
            self.lparam_to_cb.remove(h);
            self.id_to_lparam.remove(&id);
            return unregister_hot_key(hwnd, id);
        }
        Ok(())
    }

    pub fn get_callback(&mut self, lparam: u32) -> Option<&T> {
        self.lparam_to_cb.get(&lparam)
    }
}
