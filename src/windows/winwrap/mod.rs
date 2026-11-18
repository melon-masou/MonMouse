pub mod devices;
pub mod display;
pub mod inject;
pub mod input;
pub mod overlay;
pub mod startup;
pub mod window;

use windows::{
    core::HSTRING,
    Win32::{
        Devices::DeviceAndDriverInstallation::CONFIGRET,
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0},
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    },
};

use crate::errors::{Error, Result};

const STR_INVALID_WIN_WIDE_OS_STR: &str = "InvalidWinWideOsStr";

// core error
#[macro_export]
macro_rules! ce {
    ( $call:expr ) => {{
        $call.map_err(|e| core_error(e, stringify!($call)))
    }};
}

#[inline(always)]
pub fn cr_error(cr: CONFIGRET) -> Error {
    Error::WinConfigRet(cr.0)
}

#[inline(always)]
pub fn core_error(e: ::windows::core::Error, call: &'static str) -> Error {
    Error::WinCore(e.code().0, call)
}

pub fn hstring_from_buffer(buffer: &[u16]) -> Result<HSTRING> {
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    HSTRING::from_wide(&buffer[..end]).map_err(|e| core_error(e, "HSTRING::from_wide"))
}

pub fn split_wide_strings(buffer: &[u16]) -> Result<Vec<HSTRING>> {
    buffer
        .split(|c| *c == 0)
        .take_while(|part| !part.is_empty())
        .map(hstring_from_buffer)
        .collect()
}

pub fn wide_to_string(value: &HSTRING) -> String {
    // Preserve the existing device metadata display for malformed UTF-16.
    String::from_utf16(value.as_wide()).unwrap_or_else(|_| STR_INVALID_WIN_WIDE_OS_STR.to_owned())
}

pub fn get_last_error(call: &'static str) -> Error {
    let e = unsafe { GetLastError() };
    Error::WinCore(e.0 as i32, call)
}

pub struct ScopeHandle(HANDLE);

impl ScopeHandle {
    fn new(h: HANDLE) -> Self {
        ScopeHandle(h)
    }
    fn get(&self) -> &HANDLE {
        &self.0
    }
}

impl Drop for ScopeHandle {
    fn drop(&mut self) {
        let _ = close_handle(self.0);
    }
}

pub fn close_handle(handle: HANDLE) -> Result<()> {
    unsafe { ce!(CloseHandle(handle)) }
}

pub fn create_mutex(name: &HSTRING) -> Result<Option<HANDLE>> {
    match unsafe { CreateMutexW(None, false, name) } {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            if e.code() == ERROR_ALREADY_EXISTS.to_hresult() {
                Ok(None)
            } else {
                Err(core_error(e, "CreatMutexW"))
            }
        }
    }
}

pub fn try_lock_mutex(handle: HANDLE) -> bool {
    let r = unsafe { WaitForSingleObject(handle, 0) };
    r == WAIT_OBJECT_0
}

pub fn release_mutex(handle: HANDLE) -> Result<()> {
    unsafe { ce!(ReleaseMutex(handle)) }
}
