use std::sync::mpsc;

pub enum DeviceStatus {
    Active,
    Idle,
    Disconnected,
}
pub struct DeviceNotiEvent {}

pub struct WinDeviceNotifier {
    noti_tx: mpsc::Sender<DeviceNotiEvent>,
}
