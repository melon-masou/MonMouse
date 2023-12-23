use std::fmt::Display;

use crate::message::Positioning;
use crate::setting::DeviceSetting;

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

pub struct DeviceController {
    id: u64,
    setting: DeviceSetting,

    last_active_tick: u64, // in ms
    last_active_pos: MousePos,
    positioning: Positioning,
    locked_area: Option<MonitorArea>,
}

impl DeviceController {
    pub fn new(id: u64, setting: DeviceSetting) -> DeviceController {
        DeviceController {
            id,
            setting,
            last_active_tick: 0,
            last_active_pos: MousePos::default(),
            positioning: Positioning::Unknown,
            locked_area: None,
        }
    }

    pub fn update_settings(&mut self, new_setting: &DeviceSetting) {
        self.locked_area = None;
        self.setting = *new_setting;
    }

    pub fn update_positioning(&mut self, p: Positioning) {
        self.positioning = p;
    }

    pub fn reset_locked_area(&mut self) {
        self.locked_area = None;
    }

    fn update_pos(&mut self, p: &MousePos, tick: u64) {
        self.last_active_pos = *p;
        self.last_active_tick = tick;
    }

    pub fn get_last_pos(&self) -> Option<(u64, MousePos, Positioning)> {
        if self.last_active_tick > 0 {
            Some((
                self.last_active_tick,
                self.last_active_pos,
                self.positioning,
            ))
        } else {
            None
        }
    }
}

pub struct RelocatePos(pub MousePos, pub u32);

impl RelocatePos {
    pub fn from(pos: MousePos, area: &MonitorArea) -> Option<Self> {
        Some(Self(pos, area.scale))
    }
}

pub struct MouseRelocator {
    monitors: MonitorAreasList,

    cur_mouse: u64,
    cur_pos: MousePos,
    relocate_pos: Option<RelocatePos>,
    to_update_monitors: bool,
}

impl Default for MouseRelocator {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseRelocator {
    pub fn new() -> Self {
        MouseRelocator {
            monitors: MonitorAreasList::from(Vec::new()),

            cur_mouse: 0,
            cur_pos: MousePos::default(),
            relocate_pos: None,
            to_update_monitors: false,
        }
    }

    pub fn update_monitors(&mut self, monitors: MonitorAreasList) {
        self.monitors = monitors;
    }

    pub fn jump_to_next_monitor(&mut self, c: Option<&mut DeviceController>) {
        let next_id = self.monitors.locate_id(&self.cur_pos) + 1;
        if let Some(area) = self.monitors.circle_get(next_id) {
            if let Some(c) = c {
                if c.setting.locked_in_monitor {
                    c.locked_area = Some(*area);
                }
            }
            self.cur_pos = area.center();
            self.relocate_pos = RelocatePos::from(self.cur_pos, area);
        }
    }

    pub fn on_pos_update(&mut self, optc: Option<&mut DeviceController>, pos: MousePos) {
        if let Some(c) = optc {
            if c.setting.locked_in_monitor {
                // Has been locked into one area
                if let Some(area) = &c.locked_area {
                    // If leaving area
                    let new_pos = area.capture_pos(&pos);
                    if new_pos != pos {
                        self.cur_pos = new_pos;
                        self.relocate_pos = RelocatePos::from(new_pos, area);
                        return;
                    }
                } else {
                    // Find area to be locked
                    if let Some(area) = self.monitors.locate(&pos) {
                        c.locked_area = Some(*area);
                    } else {
                        self.to_update_monitors = true;
                        return;
                    }
                }
            }
        }
        self.cur_pos = pos;
    }

    pub fn on_mouse_update(&mut self, c: &mut DeviceController, tick: u64) {
        if self.cur_mouse != c.id {
            self.cur_mouse = c.id;

            if c.setting.switch {
                // Has rememberd position
                if let Some((_, old_pos, _)) = c.get_last_pos() {
                    // Find area to go
                    if let Some(area) = self.monitors.locate(&old_pos) {
                        self.cur_pos = old_pos;
                        self.relocate_pos = RelocatePos::from(old_pos, area);
                        return;
                    } else {
                        self.to_update_monitors = true;
                        return;
                    }
                }
            }
        }
        c.update_pos(&self.cur_pos, tick);
    }

    pub fn pop_relocate_pos(&mut self) -> Option<RelocatePos> {
        self.relocate_pos.take()
    }
    pub fn pop_need_update_monitors(&mut self) -> bool {
        let v = self.to_update_monitors;
        self.to_update_monitors = false;
        v
    }
}

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
    pub fn locate_id(&self, p: &MousePos) -> usize {
        if let Some((i, _)) = self.list.iter().enumerate().find(|(_, &ma)| ma.contains(p)) {
            i
        } else {
            0
        }
    }
    pub fn circle_get(&self, id: usize) -> Option<&MonitorArea> {
        self.list.get(id % self.list.len())
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

#[derive(Default, Clone, Copy)]
pub struct MonitorArea {
    pub lefttop: MousePos,
    pub rigtbtm: MousePos,
    pub scale: u32,
}

impl MonitorArea {
    pub fn contains(&self, p: &MousePos) -> bool {
        (self.lefttop.x <= p.x && p.x <= self.rigtbtm.x)
            && (self.lefttop.y <= p.y && p.y <= self.rigtbtm.y)
    }
    const RESERVE_PIXEL: i32 = 3;
    pub fn capture_pos(&self, p: &MousePos) -> MousePos {
        let rp = Self::RESERVE_PIXEL;
        let x1 = match (p.x < self.lefttop.x, p.x > self.rigtbtm.x - rp) {
            (true, _) => self.lefttop.x,
            (_, true) => self.rigtbtm.x - rp,
            _ => p.x,
        };
        let y1 = match (p.y < self.lefttop.y, p.y > self.rigtbtm.y - rp) {
            (true, _) => self.lefttop.y,
            (_, true) => self.rigtbtm.y - rp,
            _ => p.y,
        };
        MousePos::from(x1, y1)
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
            "{{{}.{}-{}.{},x{}}}",
            self.lefttop.x, self.lefttop.y, self.rigtbtm.x, self.rigtbtm.y, self.scale
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_area_capture_pos() {
        let pt = MousePos::from;
        let m = MonitorArea {
            lefttop: pt(-100, 500),
            rigtbtm: pt(300, 1500),
            scale: 100,
        };
        assert_eq!(m.capture_pos(&pt(50, 700)), pt(50, 700));
        assert_eq!(m.capture_pos(&pt(-150, 1500)), pt(-100, 1500));
        assert_eq!(m.capture_pos(&pt(350, 500)), pt(300, 500));
        assert_eq!(m.capture_pos(&pt(-100, 490)), pt(-100, 500));
        assert_eq!(m.capture_pos(&pt(300, 3000)), pt(300, 1500));
        assert_eq!(m.capture_pos(&pt(-120, 1300)), pt(-100, 1300));
        assert_eq!(m.capture_pos(&pt(-200, 1800)), pt(-100, 1500));
    }
}
