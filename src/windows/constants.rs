pub const STR_INVALID_WIN_WIDE_OS_STR: &str = "InvalidWinWideOsStr";

pub const RATELIMIT_UPDATE_MONITOR_ONCE_MS: u64 = 1000;
pub const RATELIMIT_UPDATE_DEVICE_ONCE_MS: u64 = 1000;
pub const MOUSE_EVENT_ACTIVE_LAST_FOR_MS: u64 = 100;

pub const WIN_EVENTLOOP_POLL_MAX_MESSAGES: u32 = 20;
pub const WIN_EVENTLOOP_POLL_WAIT_TIMEOUT_MS: u32 = 20;
pub const RAWINPUT_MSG_INIT_BUF_SIZE: u32 = 1024;
pub const RAWINPUT_MOUSE_FLAGS_ABSOLUTE: u16 = 1;

#[repr(i32)]
#[derive(Clone, Copy)]
pub enum ShortcutID {
    CurMouseLock = 1000,
    CurMouseJumpNext = 1001,
}
