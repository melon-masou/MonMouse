use crate::input_control::{DeviceId, MonitorAreasList, MousePos};
use crate::{device_type::DeviceType, errors::Result};

// A discovered source can lack a persistent ID while still producing input.
pub struct DeviceInfo {
    pub source: DeviceId, // Runtime identity; zero represents an unidentified device source.
    pub id: Option<String>,
    pub device_type: DeviceType,
    pub product_name: String,
    pub platform_specific_infos: Vec<(String, String)>,
}

pub trait PlatformService {
    fn register_raw_devices(&mut self) -> Result<()>;
    fn scan_devices(&mut self) -> Result<Vec<DeviceInfo>>;
    fn get_monitors(&mut self) -> Result<MonitorAreasList>;
    fn register_shortcut(&mut self, text: &str) -> Result<i32>;
    fn unregister_shortcut(&mut self, id: i32) -> Result<()>;
    fn refresh_overlay_window(&mut self) -> Result<()>;
    fn draw_cursors(&mut self, positions: Vec<(MousePos, usize)>, show_marker: bool);
    fn set_cursor_pos(&mut self, position: MousePos) -> Result<()>;
    fn get_cur_tick(&self) -> u64;
}

#[cfg(target_os = "windows")]
pub use crate::windows::locate_file;
#[cfg(target_os = "windows")]
pub type Eventloop = crate::windows::poller::Poller;
#[cfg(target_os = "windows")]
pub type SingleProcess = crate::windows::SingleProcess;
