use super::{DeviceId, MousePos, Positioning};

#[derive(Clone, Copy, Debug)]
pub struct MatchedMouseEvent {
    pub device: DeviceId,
    pub position: MousePos,
    pub positioning: Positioning,
    pub tick: u64,
}

#[derive(Clone, Copy)]
struct DeviceActivity {
    device: DeviceId,
    positioning: Positioning,
    tick: u64,
}

// On Windows, Cursor position events may lack a device identity.
// MouseEventDeviceMatcher try to infer their source from recent device activity.
pub struct MouseEventDeviceMatcher {
    // Overlapping device activity makes the latest observation a candidate for a position event.
    last_activity: Option<DeviceActivity>,
    // Merge unidentified device activity into the last source within this many ms; -1 disables merging.
    merge_unassociated_ms: i64,
}

impl MouseEventDeviceMatcher {
    pub fn new(merge_unassociated_ms: i64) -> Self {
        Self {
            last_activity: None,
            merge_unassociated_ms,
        }
    }

    pub fn set_merge_unassociated_ms(&mut self, value: i64) {
        self.merge_unassociated_ms = value;
    }

    pub fn reset(&mut self) {
        self.last_activity = None;
    }

    pub fn observe_device(
        &mut self,
        mut device: Option<DeviceId>,
        positioning: Positioning,
        tick: u64,
    ) -> DeviceId {
        if device.is_none() && self.merge_unassociated_ms >= 0 {
            if let Some(previous) = self.last_activity {
                if previous.tick + self.merge_unassociated_ms as u64 >= tick {
                    device = Some(previous.device);
                }
            }
        }
        let device = device.unwrap_or(0);
        self.last_activity = Some(DeviceActivity {
            device,
            positioning,
            tick,
        });
        device
    }

    pub fn match_position(&self, position: MousePos, tick: u64) -> Option<MatchedMouseEvent> {
        let activity = self.last_activity?;
        Some(MatchedMouseEvent {
            device: activity.device,
            position,
            positioning: activity.positioning,
            tick,
        })
    }
}
