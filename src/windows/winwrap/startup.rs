use std::ffi::OsString;

use windows::{
    core::{w, HSTRING, PCWSTR},
    Win32::{
        Foundation::ERROR_FILE_NOT_FOUND,
        System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ},
    },
};

use crate::errors::{Error, Result};

use super::core_error;

pub fn set_start_on_login(enabled: bool, name: PCWSTR) -> Result<()> {
    let key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    // All copies use the same value; saving from another location replaces its path.
    if enabled {
        let executable = std::env::current_exe().map_err(Error::IO)?;
        let mut command = OsString::from("\"");
        command.push(executable);
        command.push("\"");
        let command = HSTRING::from(command);
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key,
                name,
                REG_SZ.0,
                Some(command.as_ptr().cast()),
                ((command.len() + 1) * size_of::<u16>()) as u32,
            )
        }
        .ok()
        .map_err(|e| core_error(e, "RegSetKeyValueW"))
    } else {
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key, name) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        status.ok().map_err(|e| core_error(e, "RegDeleteKeyValueW"))
    }
}
