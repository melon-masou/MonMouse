pub mod arbiter;
pub mod mouse_event_device_matcher;
pub mod relocator;

pub use monmouse_lib::input::{
    DeviceId, Key, MatchedMouseEvent, MonitorArea, MonitorAreasList, MouseAction, MouseButton,
    MousePos, NormalizedMousePos, Positioning, SysKeyboardEvent, SysMouseEvent,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_area_capture_pos() {
        let pt = MousePos::from;
        let m = MonitorArea::new(pt(-100, 500), pt(301, 1501));
        assert_eq!(m.capture_pos(&pt(50, 700)), pt(50, 700));
        assert_eq!(m.capture_pos(&pt(-150, 1500)), pt(-100, 1500));
        assert_eq!(m.capture_pos(&pt(350, 500)), pt(300, 500));
        assert_eq!(m.capture_pos(&pt(-100, 490)), pt(-100, 500));
        assert_eq!(m.capture_pos(&pt(300, 3000)), pt(300, 1500));
        assert_eq!(m.capture_pos(&pt(-120, 1300)), pt(-100, 1300));
        assert_eq!(m.capture_pos(&pt(-200, 1800)), pt(-100, 1500));
    }
}
