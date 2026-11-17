pub mod constants;
mod devices;
pub mod eventloop;
mod poller;
mod service;
pub mod winwrap;

use crate::errors::Error;
use windows::core::HSTRING;
use windows::Win32::Foundation::HANDLE;

use self::winwrap::{close_handle, create_mutex, release_mutex, try_lock_mutex};

pub fn locate_file(path: &std::path::Path) -> Result<(), Error> {
    use std::os::windows::process::CommandExt;

    path.metadata().map_err(Error::IO)?;
    let path = std::path::absolute(path).map_err(Error::IO)?;
    std::process::Command::new("explorer.exe")
        .raw_arg("/select,")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(Error::IO)
}

#[derive(Debug)]
pub struct SingleProcess {
    handle: HANDLE,
}

impl SingleProcess {
    pub fn create() -> Result<Self, Error> {
        Self::new("Global\\MonmouseSingleProcessMutex")
    }

    fn new(mutex_name: &str) -> Result<Self, Error> {
        let handle = match create_mutex(&HSTRING::from(mutex_name)) {
            Ok(Some(handle)) => handle,
            Ok(None) => return Err(Error::AlreadyLaunched),
            Err(e) => return Err(e),
        };
        if !try_lock_mutex(handle) {
            let _ = close_handle(handle);
            Err(Error::AlreadyLaunched)
        } else {
            Ok(Self { handle })
        }
    }
}

impl Drop for SingleProcess {
    fn drop(&mut self) {
        let _ = release_mutex(self.handle);
        let _ = close_handle(self.handle);
    }
}
