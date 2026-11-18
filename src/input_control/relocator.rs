use super::{
    mouse_event_device_matcher::MatchedMouseEvent, DeviceId, MonitorArea, MonitorAreasList,
    MousePos, Positioning,
};
use crate::{setting::DeviceSetting, utils::vec_ensure_get_mut};

const MOUSE_SWITCHING_MS: u64 = 100;
const MOUSE_SWITCHING_JUMP_RESET_PX: i32 = 200;

#[derive(Debug)]
pub struct MouseDeviceTracker {
    pub id: DeviceId,
    pub setting: DeviceSetting,

    last_active_tick: u64, // in ms
    last_active_pos: MousePos,

    positioning: Positioning,
    locked_area: Option<MonitorArea>,
}

impl MouseDeviceTracker {
    pub fn new(id: DeviceId, setting: DeviceSetting) -> MouseDeviceTracker {
        MouseDeviceTracker {
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

    pub fn reset(&mut self) {
        self.locked_area = None;
        self.last_active_tick = 0;
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

#[derive(Default, Clone, Copy)]
pub struct RelocatorEffect {
    pub switch: bool,
    pub refresh_monitors: bool,
    pub relocate_pos: Option<MousePos>,
}

pub struct MouseRelocator {
    monitors: MonitorAreasList,

    cur_mouse: DeviceId,
    cur_pos: MousePos,
    last_jump_pos: Vec<Option<MousePos>>,
    switching_until: u64,
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
            last_jump_pos: Vec::new(),
            switching_until: 0,
        }
    }

    pub fn update_monitors(&mut self, monitors: MonitorAreasList) {
        self.monitors = monitors;
        // clear previous state
        self.last_jump_pos.fill(None);
    }

    pub fn monitors(&self) -> &MonitorAreasList {
        &self.monitors
    }

    pub fn current_position(&self) -> MousePos {
        self.cur_pos
    }

    pub fn initialize_position(&mut self, position: MousePos) {
        self.cur_pos = position;
    }

    fn is_switching(&self, tick: u64) -> bool {
        tick <= self.switching_until
    }

    // On Windows, switching to a remembered cursor position is not completed by
    // a single SetCursorPos call. Mouse input already queued by Windows or the
    // device driver can still move the cursor after relocation.
    //
    // We handle this with a short switching window. During this window, small
    // cursor movements are accepted. If the cursor jumps far away from cur_pos,
    // we treat it as a coordinate stream from before the switch and request
    // another relocation back to cur_pos. Current mouse updates are ignored
    // during the window so boundary events cannot trigger another switch or
    // overwrite saved_pos.
    fn start_switching(&mut self, tick: u64) {
        self.switching_until = tick.saturating_add(MOUSE_SWITCHING_MS);
    }

    fn is_big_switching_jump(&self, pos: &MousePos) -> bool {
        let dx = i64::from(pos.x) - i64::from(self.cur_pos.x);
        let dy = i64::from(pos.y) - i64::from(self.cur_pos.y);
        let threshold = i64::from(MOUSE_SWITCHING_JUMP_RESET_PX);
        dx * dx + dy * dy > threshold * threshold
    }

    pub fn jump_to_next_monitor(
        &mut self,
        ctrl: Option<&mut MouseDeviceTracker>,
    ) -> RelocatorEffect {
        if self.monitors.is_empty() {
            return RelocatorEffect {
                refresh_monitors: true,
                ..Default::default()
            };
        }
        let next_id = if let Some((cur_id, cur_mon)) = self.monitors.locate_id(&self.cur_pos) {
            let saved_cur_pos = cur_mon.capture_pos_no_edge(&self.cur_pos);
            *vec_ensure_get_mut(&mut self.last_jump_pos, cur_id) = Some(saved_cur_pos);
            self.monitors.next_id(cur_id)
        } else {
            0 // maybe go to primary monitor?
        };

        let Some(area) = self.monitors.get_area(next_id) else {
            return RelocatorEffect {
                refresh_monitors: true,
                ..Default::default()
            };
        };
        let mut new_pos = area.center();
        if let Some(ctrl) = ctrl {
            if ctrl.setting.locked_in_monitor {
                // Clear and find new one in next mouse event. In case user requests
                // jumping at the edge of monitor, which is hard to say locked to
                // which monitor.
                ctrl.locked_area = None;
            }
            if let Some(Some(pos)) = self.last_jump_pos.get(next_id) {
                new_pos = *pos;
            }
        }
        self.cur_pos = new_pos;
        RelocatorEffect {
            relocate_pos: Some(new_pos),
            ..Default::default()
        }
    }

    pub fn on_mouse_event(
        &mut self,
        event: &MatchedMouseEvent,
        ctrl: &mut MouseDeviceTracker,
    ) -> RelocatorEffect {
        let mut effect = RelocatorEffect::default();

        ctrl.positioning = event.positioning;
        if self.is_switching(event.tick) {
            if self.is_big_switching_jump(&event.position) {
                effect.relocate_pos = Some(self.cur_pos);
                log::trace!(
                    "Reset cursor during mouse switching: event_pos={}, cur_pos={}, tick={}, switching_until={}",
                    event.position,
                    self.cur_pos,
                    event.tick,
                    self.switching_until
                );
            } else {
                self.update_position(ctrl, &mut effect, event.position);
            }
            return effect;
        }

        let switched = self.cur_mouse != event.device;
        self.cur_mouse = event.device;
        if let Some((_, saved_pos, _)) = ctrl
            .get_last_pos()
            .filter(|_| switched && ctrl.setting.switch)
        {
            // The incoming position still belongs to the stream before relocation.
            self.cur_pos = saved_pos;
            effect.relocate_pos = Some(saved_pos);
        } else {
            self.update_position(ctrl, &mut effect, event.position);
        }
        ctrl.update_pos(&self.cur_pos, event.tick);
        if switched {
            self.start_switching(event.tick);
        }
        effect.switch = switched;
        effect
    }

    fn update_position(
        &mut self,
        ctrl: &mut MouseDeviceTracker,
        effect: &mut RelocatorEffect,
        pos: MousePos,
    ) {
        if ctrl.setting.locked_in_monitor {
            if let Some(area) = &ctrl.locked_area {
                let new_pos = area.capture_pos(&pos);
                if new_pos != pos {
                    self.cur_pos = new_pos;
                    effect.relocate_pos = Some(new_pos);
                    return;
                }
            } else if let Some(area) = self.monitors.locate(&pos) {
                ctrl.locked_area = Some(*area);
            } else {
                effect.refresh_monitors = true;
                return;
            }
        }
        self.cur_pos = pos;
    }
}
