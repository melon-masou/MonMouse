pub mod errors;
pub mod message;
pub mod mouse_control;
pub mod utils;

#[cfg(target_os = "windows")]
#[path = "windows/mod.rs"]
pub mod windows;

#[cfg(target_os = "windows")]
pub type Eventloop = windows::win_processor::WinEventLoop;
