const GLOBAL_SCALE: f32 = 1.1;

#[inline]
pub fn gscale(v: f32) -> f32 {
    v * GLOBAL_SCALE
}

#[derive(Debug)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn from_string(s: &str) -> Self {
        match s {
            "Light" => Theme::Light,
            "Dark" => Theme::Dark,
            _ => Theme::Light,
        }
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", *self)
    }
}
