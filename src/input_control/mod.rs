pub mod arbiter;
pub mod mouse_event_device_matcher;
pub mod relocator;

pub use monmouse_lib::input::{
    DeviceId, Key, MatchedMouseEvent, MonitorArea, MonitorAreasList, MouseAction, MouseButton,
    MousePos, NormalizedMousePos, Positioning, SysKeyboardEvent, SysMouseEvent,
};

use std::collections::BTreeSet;

// Keys and mouse buttons held by one input source.
#[derive(Clone, Debug, Default)]
pub struct HeldInput {
    keys: BTreeSet<Key>,
    buttons: BTreeSet<MouseButton>,
}

impl HeldInput {
    pub fn pressed(&self) -> bool {
        !self.keys.is_empty() || !self.buttons.is_empty()
    }

    pub fn key(&mut self, key: Key, down: bool) -> bool {
        if down {
            self.keys.insert(key);
            true
        } else {
            self.keys.remove(&key)
        }
    }

    pub fn button(&mut self, button: MouseButton, down: bool) -> bool {
        if down {
            self.buttons.insert(button);
            true
        } else {
            self.buttons.remove(&button)
        }
    }

    pub fn exclude(&mut self, other: &Self) {
        self.keys.retain(|key| !other.keys.contains(key));
        self.buttons
            .retain(|button| !other.buttons.contains(button));
    }

    pub fn release(&mut self) -> (BTreeSet<Key>, BTreeSet<MouseButton>) {
        let held = std::mem::take(self);
        (held.keys, held.buttons)
    }
}

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
