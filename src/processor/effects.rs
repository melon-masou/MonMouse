use crate::input_control::MousePos;

#[derive(Clone, Copy)]
pub struct RelocateIntent {
    pub position: MousePos,
    pub timestamp: u64,
}

// MessageDispatch owns this pending work. Handlers borrow it only to record
// changes; output is taken before native calls can record further work.
#[derive(Default, Clone, Copy)]
pub struct EffectsIntent {
    pub refresh_devices: bool,
    pub refresh_monitors: bool,
    pub refresh_overlay: bool,
    pub redraw_inactive_cursors: bool,
    pub relocation: Option<RelocateIntent>,
}

impl EffectsIntent {
    pub fn has_pending(&self) -> bool {
        self.refresh_devices
            || self.refresh_monitors
            || self.refresh_overlay
            || self.redraw_inactive_cursors
            || self.relocation.is_some()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn relocate(&mut self, position: MousePos, timestamp: u64) {
        if self
            .relocation
            .is_none_or(|current| timestamp > current.timestamp)
        {
            self.relocation = Some(RelocateIntent {
                position,
                timestamp,
            });
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.refresh_devices |= other.refresh_devices;
        self.refresh_monitors |= other.refresh_monitors;
        self.refresh_overlay |= other.refresh_overlay;
        self.redraw_inactive_cursors |= other.redraw_inactive_cursors;
        if let Some(relocation) = other.relocation {
            self.relocate(relocation.position, relocation.timestamp);
        }
    }
}

pub struct CursorDrawing {
    pub positions: Vec<(MousePos, usize)>,
    pub show_markers: bool,
}
