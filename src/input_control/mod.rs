pub mod arbiter;
pub mod relocator;

use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
pub enum Positioning {
    Unknown,
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Key {
    pub scan: u16,
    pub virtual_key: u16,
    pub extended: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SysMouseEvent {
    pub pos_x: i32,
    pub pos_y: i32,
    pub time: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct SysKeyboardEvent {
    pub time: u32,
    pub key: Key,
    pub down: bool,
    pub injected: bool,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MousePos {
    pub x: i32,
    pub y: i32,
}

impl MousePos {
    pub fn from(x: i32, y: i32) -> Self {
        MousePos { x, y }
    }
}

impl Display for MousePos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.x, self.y)
    }
}

// Position within a screen, scaled to 0..=65535 independently of its origin and size.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedMousePos {
    pub x: u16,
    pub y: u16,
}

impl NormalizedMousePos {
    pub fn from_mouse_pos(p: &MousePos, area: &MonitorArea) -> Option<Self> {
        if !area.contains(p) {
            return None;
        }
        let normalize = |value: i32, start: i32, end: i32| {
            let offset = i64::from(value) - i64::from(start);
            let span = (i64::from(end) - i64::from(start) - 1).max(1);
            ((offset * 65535) / span) as u16
        };
        Some(Self {
            x: normalize(p.x, area.lefttop.x, area.rigtbtm.x),
            y: normalize(p.y, area.lefttop.y, area.rigtbtm.y),
        })
    }

    pub fn to_mouse_pos(self, area: &MonitorArea) -> MousePos {
        let position = |value: u16, start: i32, end: i32| {
            let span = (i64::from(end) - i64::from(start) - 1).max(0);
            (i64::from(start) + (i64::from(value) * span + 32767) / 65535) as i32
        };
        MousePos::from(
            position(self.x, area.lefttop.x, area.rigtbtm.x),
            position(self.y, area.lefttop.y, area.rigtbtm.y),
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MonitorAreasList {
    list: Vec<MonitorArea>,
}

impl MonitorAreasList {
    pub fn from(list: Vec<MonitorArea>) -> Self {
        MonitorAreasList { list }
    }
    pub fn locate(&self, p: &MousePos) -> Option<&MonitorArea> {
        self.list.iter().find(|&ma| ma.contains(p))
    }
    pub fn locate_id(&self, p: &MousePos) -> Option<(usize, &MonitorArea)> {
        self.list.iter().enumerate().find(|(_, &ma)| ma.contains(p))
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
    #[inline]
    pub fn next_id(&self, round_id: usize) -> usize {
        (round_id + 1) % self.list.len()
    }
    pub fn get_area(&self, round_id: usize) -> Option<&MonitorArea> {
        self.list.get(round_id % self.list.len())
    }
}

impl Display for MonitorAreasList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for m in self.list.iter() {
            write!(f, "{} ", m)?;
        }
        write!(f, "]")
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorArea {
    pub lefttop: MousePos,
    pub rigtbtm: MousePos,
    edge_reserve: i32,
}

impl MonitorArea {
    const EDGE_RESERVE_PIXEL: i32 = 10;
    pub fn new(lefttop: MousePos, rigtbtm: MousePos) -> MonitorArea {
        let edge_reserve = std::cmp::min(
            Self::EDGE_RESERVE_PIXEL,
            std::cmp::min(rigtbtm.x - lefttop.x, rigtbtm.y - lefttop.y) / 10,
        );

        MonitorArea {
            lefttop,
            rigtbtm,
            edge_reserve,
        }
    }
    pub fn contains(&self, p: &MousePos) -> bool {
        (self.lefttop.x <= p.x && p.x < self.rigtbtm.x)
            && (self.lefttop.y <= p.y && p.y < self.rigtbtm.y)
    }
    fn capture_pos_resv(&self, p: &MousePos, reserve_lt: i32, reserve_rb: i32) -> MousePos {
        // The value will prevent mouse moving to the edge of monitor, which is hard to say belonging to which
        // monitor. But it will cause the cursor hardly reaching the bottom of screen, affecting displaying hidden taskbar.
        let rp_min: i32 = reserve_lt;
        let rp_max: i32 = reserve_rb + 1 /* right-open */;

        let x1 = match (p.x < self.lefttop.x + rp_min, p.x > self.rigtbtm.x - rp_max) {
            (true, _) => self.lefttop.x + rp_min,
            (_, true) => self.rigtbtm.x - rp_max,
            _ => p.x,
        };
        let y1 = match (p.y < self.lefttop.y + rp_min, p.y > self.rigtbtm.y - rp_max) {
            (true, _) => self.lefttop.y + rp_min,
            (_, true) => self.rigtbtm.y - rp_max,
            _ => p.y,
        };
        MousePos::from(x1, y1)
    }

    pub fn capture_pos_no_edge(&self, p: &MousePos) -> MousePos {
        self.capture_pos_resv(p, self.edge_reserve, self.edge_reserve)
    }
    pub fn capture_pos(&self, p: &MousePos) -> MousePos {
        self.capture_pos_resv(p, 0, 0)
    }
    pub fn center(&self) -> MousePos {
        MousePos::from(
            (self.lefttop.x + self.rigtbtm.x) / 2,
            (self.lefttop.y + self.rigtbtm.y) / 2,
        )
    }
}

impl Display for MonitorArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{{}.{}-{}.{}}}",
            self.lefttop.x, self.lefttop.y, self.rigtbtm.x, self.rigtbtm.y,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Button {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseAction {
    Move,
    Button { button: Button, down: bool },
    Wheel { delta: i16, horizontal: bool },
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
