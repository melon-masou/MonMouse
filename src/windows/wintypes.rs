use windows::{core::HSTRING, Win32::Devices::DeviceAndDriverInstallation::CONFIGRET};

use crate::errors::{Error, Result};

use super::constants::STR_INVALID_WIN_WIDE_OS_STR;

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

pub(super) fn hstring_from_buffer(buffer: &[u16]) -> Result<HSTRING> {
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    HSTRING::from_wide(&buffer[..end]).map_err(|e| core_error(e, "HSTRING::from_wide"))
}

pub(super) fn split_wide_strings(buffer: &[u16]) -> Result<Vec<HSTRING>> {
    buffer
        .split(|c| *c == 0)
        .take_while(|part| !part.is_empty())
        .map(hstring_from_buffer)
        .collect()
}

pub(super) fn wide_to_string(value: &HSTRING) -> String {
    // Preserve the existing device metadata display for malformed UTF-16.
    String::from_utf16(value.as_wide()).unwrap_or_else(|_| STR_INVALID_WIN_WIDE_OS_STR.to_owned())
}
