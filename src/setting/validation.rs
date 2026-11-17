use validator::ValidationErrors;

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
