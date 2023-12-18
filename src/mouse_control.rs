use std::fmt::Display;

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

#[derive(Clone, Copy)]
pub struct DeviceCtrlSetting {
    pub restrict_in_monitor: bool,
    pub remember_pos: bool,
}

pub struct DeviceController {
    id: u64,
    setting: DeviceCtrlSetting,

    last_active_tick: u64, // in ms
    last_active_pos: MousePos,
    restric_area: Option<MonitorArea>,
}

impl DeviceController {
    pub fn new(id: u64, setting: DeviceCtrlSetting) -> DeviceController {
        DeviceController {
            id,
            setting,
            last_active_tick: 0,
            last_active_pos: MousePos::default(),
            restric_area: None,
        }
    }

    pub fn update_settings(&mut self, new_setting: &DeviceCtrlSetting) {
        self.reset_restric_area();
        self.setting = *new_setting;
    }
    pub fn reset_restric_area(&mut self) {
        self.restric_area = None;
    }

    fn update_pos(&mut self, p: &MousePos, tick: u64) {
        self.last_active_pos = *p;
        self.last_active_tick = tick;
    }

    fn get_last_pos(&self) -> Option<MousePos> {
        if self.last_active_tick > 0 {
            Some(self.last_active_pos)
        } else {
            None
        }
    }
}

pub struct MouseRelocator {
    monitors: MonitorAreasList,

    cur_mouse: u64,
    cur_pos: MousePos,
    relocate_pos: Option<(MousePos, u32)>,
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

    pub fn on_pos_update(&mut self, optc: Option<&mut DeviceController>, pos: MousePos) {
        if let Some(c) = optc {
            if c.setting.restrict_in_monitor {
                // Has been restricted into one area
                if let Some(area) = &c.restric_area {
                    // If leaving area
                    let new_pos = area.capture_pos(&pos);
                    if new_pos != pos {
                        self.relocate_pos = Some((new_pos, area.scale));
                        return;
                    }
                } else {
                    // Find area to be restricted
                    if let Some(area) = self.monitors.locate(&pos) {
                        c.restric_area = Some(*area);
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

            if c.setting.remember_pos {
                // Has rememberd position
                if let Some(old_pos) = c.get_last_pos() {
                    // Find area to go
                    if let Some(area) = self.monitors.locate(&old_pos) {
                        self.relocate_pos = Some((old_pos, area.scale));
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

    pub fn pop_relocate_pos(&mut self) -> Option<(MousePos, u32)> {
        self.relocate_pos.take()
    }

    pub fn need_update_monitors(&mut self) -> bool {
        self.to_update_monitors
    }
    pub fn done_update_monitors(&mut self) {
        self.to_update_monitors = false;
    }
}

pub struct MonitorAreasList {
    pub list: Vec<MonitorArea>,
}

impl MonitorAreasList {
    pub fn from(list: Vec<MonitorArea>) -> Self {
        MonitorAreasList { list }
    }
    pub fn locate(&self, p: &MousePos) -> Option<&MonitorArea> {
        self.list.iter().find(|&ma| ma.contains(p))
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
    pub fn capture_pos(&self, p: &MousePos) -> MousePos {
        let x1 = match (p.x < self.lefttop.x, p.x > self.rigtbtm.x) {
            (true, _) => self.lefttop.x,
            (_, true) => self.rigtbtm.x,
            _ => p.x,
        };
        let y1 = match (p.y < self.lefttop.y, p.y > self.rigtbtm.y) {
            (true, _) => self.lefttop.y,
            (_, true) => self.rigtbtm.y,
            _ => p.y,
        };
        MousePos::from(x1, y1)
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