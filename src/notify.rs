use std::sync::mpsc;

pub struct DeviceNotiEvent {}

pub struct WinDeviceNotifier {
    noti_tx: mpsc::Sender<DeviceNotiEvent>,
}
