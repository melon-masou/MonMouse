#[derive(Clone, Copy, Debug)]
pub struct DeviceSetting {
    pub locked_in_monitor: bool,
    pub switch: bool,
}

pub struct Settings {
    pub merge_unassociated_events_within_next_ms: Option<u64>,
    pub devices: Vec<(String, DeviceSetting)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            merge_unassociated_events_within_next_ms: Some(5),
            devices: Vec::new(),
        }
    }
}
