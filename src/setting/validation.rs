use validator::{ValidationError, ValidationErrors};

pub struct ValidatedField<'a, T> {
    pub id: &'static str,
    pub value: &'a mut T,
    pub validator: fn(&T) -> Result<(), ValidationErrors>,
}

impl<T> ValidatedField<'_, T> {
    pub fn validate(&self, value: &T) -> Result<(), ValidationErrors> {
        (self.validator)(value)
    }
}

pub fn shortcut<const ALLOW_MOUSE: bool>(value: &str) -> Result<(), ValidationError> {
    // Empty strings leave optional shortcut bindings unassigned.
    if value.is_empty() {
        return Ok(());
    }
    let result = if ALLOW_MOUSE {
        monmouse_lib::keyboard::parse_chord(value, true).map(|_| ())
    } else {
        monmouse_lib::keyboard::shortcut_from_str(value)
            .map(|_| ())
            .ok_or_else(|| "Expected a single key with optional Ctrl, Alt, Shift or Win".to_owned())
    };
    result.map_err(|error| ValidationError::new("shortcut").with_message(error.into()))
}
