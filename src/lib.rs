pub mod device_type;
pub mod errors;
pub mod keyboard;
pub mod message;
pub mod mouse_control;
pub mod setting;
pub mod utils;

pub use platform::*;

#[cfg(target_os = "windows")]
#[path = "windows/mod.rs"]
pub mod windows;

#[cfg(target_os = "windows")]
pub mod platform {
    use super::windows;
    pub type Eventloop = windows::win_processor::WinEventLoop;
    pub type SingleProcess = windows::SingleProcess;
    pub const POLL_MSGS: u32 = windows::constants::WIN_EVENTLOOP_POLL_MAX_MESSAGES;
    pub const POLL_TIMEOUT: u32 = windows::constants::WIN_EVENTLOOP_POLL_WAIT_TIMEOUT_MS;
}
