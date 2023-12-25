pub mod constants;
pub mod win_processor;
pub mod wintypes;
pub mod winwrap;

use crate::errors::Error;
use windows::Win32::Foundation::HANDLE;

use self::{
    wintypes::WString,
    winwrap::{close_handle, create_mutex, release_mutex, try_lock_mutex},
};

#[derive(Debug)]
pub struct SingleProcess {
    handle: HANDLE,
}

impl SingleProcess {
    pub fn create() -> Result<Self, Error> {
        Self::new("Global\\MonmouseSingleProcessMutex")
    }

    fn new(mutex_name: &str) -> Result<Self, Error> {
        let handle = match create_mutex(WString::encode_from_str(mutex_name)) {
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
