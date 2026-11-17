pub mod device_type;
pub mod errors;
pub mod input_control;
pub mod keyboard;
pub mod message;
pub(crate) mod processor;
pub mod setting;
pub mod utils;

pub use platform::*;

#[cfg(target_os = "windows")]
#[path = "windows/mod.rs"]
pub mod windows;

#[cfg(target_os = "windows")]
pub mod platform {
    use super::windows;
    pub use windows::locate_file;
    pub type Eventloop = windows::eventloop::WinEventLoop;
    pub type SingleProcess = windows::SingleProcess;
    pub const POLL_MSGS: u32 = crate::processor::POLL_MAX_MESSAGES;
    pub const POLL_TIMEOUT: u32 = crate::processor::POLL_WAIT_TIMEOUT_MS;
}
