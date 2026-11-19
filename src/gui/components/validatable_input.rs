use std::{borrow::Cow, collections::HashMap, fmt::Display, str::FromStr};

use eframe::egui;
use monmouse::setting::validation::ValidatedField;
use validator::{Validate, ValidationError, ValidationErrors, ValidationErrorsKind};

use super::widget::{error_color, icon_button};

pub struct InputItem<'a, F> {
    text: Cow<'a, str>,
    tooltip: Option<Cow<'a, str>>,
    field: F,
}

impl<'a, F: InputField> InputItem<'a, F> {
    pub fn new(text: impl Into<Cow<'a, str>>, field: F) -> Self {
        Self {
            text: text.into(),
            tooltip: None,
            field,
        }
    }

    pub fn with_tip(mut self, tooltip: impl Into<Cow<'a, str>>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn show(
        mut self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui, &mut F::Value) -> bool,
    ) {
        input_label(ui, self.text.as_ref(), self.tooltip.as_deref());
        let changed = add_contents(ui, self.field.value());
        if let Some(error) = self.field.on_field_update(changed) {
            ui.colored_label(error_color(ui, false), error);
        }
        ui.end_row();
    }
}

pub fn input_label(ui: &mut egui::Ui, text: &str, tooltip: Option<&str>) {
    if let Some(tooltip) = tooltip {
        ui.horizontal(|ui| {
            ui.label(text);
            let help = ui
                .add(icon_button(ui, egui::RichText::new("?").small()).frame(false))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            egui::Popup::from_toggle_button_response(&help)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .width(300.0)
                .show(|ui| {
                    ui.add(egui::Label::new(tooltip).wrap());
                });
        });
    } else {
        ui.label(text);
    }
}

pub trait InputField {
    type Value;

    fn value(&mut self) -> &mut Self::Value;
    fn on_field_update(self, changed: bool) -> Option<String>;
}

pub struct InputBinding<'a, T> {
    field: ValidatedField<'a, T>,
    state: &'a mut InputState,
}

impl<T> InputField for InputBinding<'_, T> {
    type Value = T;

    fn value(&mut self) -> &mut T {
        self.field.value
    }

    fn on_field_update(self, changed: bool) -> Option<String> {
        let error = self
            .field
            .validate(self.field.value)
            .err()
            .map(validation_error_text);
        self.state.on_field_update(self.field.id, changed, error)
    }
}

pub struct InputAsTextBinding<'a, T> {
    input: InputBinding<'a, T>,
    buffer: &'a mut String,
}

impl<T: FromStr> InputField for InputAsTextBinding<'_, T> {
    type Value = String;

    fn value(&mut self) -> &mut String {
        self.buffer
    }

    fn on_field_update(self, changed: bool) -> Option<String> {
        let error = match self.buffer.parse::<T>() {
            Ok(value) => match self.input.field.validate(&value) {
                Ok(()) => {
                    *self.input.field.value = value;
                    None
                }
                Err(errors) => Some(validation_error_text(errors)),
            },
            Err(_) => Some(t!("config.validate.invalid_value").to_string()),
        };
        self.input
            .state
            .on_field_update(self.input.field.id, changed, error)
    }
}

// Separate update state from values so a binding can borrow both without retaining a selector.
#[derive(Default)]
struct InputState {
    errors: HashMap<&'static str, Option<String>>,
    changed: bool,
}

impl InputState {
    fn on_field_update(
        &mut self,
        id: &'static str,
        changed: bool,
        error: Option<String>,
    ) -> Option<String> {
        self.changed |= changed;
        self.errors.insert(id, error.clone());
        error
    }
}

// Own the draft values and validation results for every control.
#[derive(Default)]
pub struct InputsCache<T> {
    object: T,
    texts: HashMap<&'static str, String>,
    state: InputState,
}

impl<T> InputsCache<T> {
    pub fn new(object: T) -> Self {
        Self {
            object,
            texts: HashMap::new(),
            state: InputState::default(),
        }
    }

    pub fn object(&self) -> &T {
        &self.object
    }

    pub fn object_mut(&mut self) -> &mut T {
        &mut self.object
    }

    pub fn field<'a, V>(
        &'a mut self,
        select: impl FnOnce(&'a mut T) -> ValidatedField<'a, V>,
    ) -> InputBinding<'a, V> {
        InputBinding {
            field: select(&mut self.object),
            state: &mut self.state,
        }
    }

    pub fn text_field<'a, V: Display + FromStr>(
        &'a mut self,
        select: impl FnOnce(&'a mut T) -> ValidatedField<'a, V>,
    ) -> InputAsTextBinding<'a, V> {
        let field = select(&mut self.object);
        let buffer = self
            .texts
            .entry(field.id)
            .or_insert_with(|| field.value.to_string());
        InputAsTextBinding {
            input: InputBinding {
                field,
                state: &mut self.state,
            },
            buffer,
        }
    }

    pub fn has_errors(&self) -> bool {
        self.state.errors.values().any(Option::is_some)
    }

    pub fn has_changes(&self) -> bool {
        self.state.changed
    }

    pub fn clear_changes(&mut self) {
        self.state.changed = false;
    }

    pub fn replace(&mut self, object: T) {
        *self.object_mut() = object;
        self.texts.clear();
        self.state = InputState {
            changed: true,
            ..Default::default()
        };
    }
}

impl<T: Validate> InputsCache<T> {
    pub fn validate(&self) -> bool {
        self.object.validate().is_ok() && !self.has_errors()
    }
}

fn validation_error_text(errors: ValidationErrors) -> String {
    let mut messages = Vec::new();
    for errors in errors.into_errors().into_values() {
        match errors {
            ValidationErrorsKind::Field(errors) => {
                messages.extend(errors.iter().map(field_error_text));
            }
            ValidationErrorsKind::Struct(errors) => messages.push(validation_error_text(*errors)),
            ValidationErrorsKind::List(errors) => {
                for (index, errors) in errors {
                    messages.push(format!("{}: {}", index, validation_error_text(*errors)));
                }
            }
        }
    }
    messages.join("\n")
}

fn field_error_text(error: &ValidationError) -> String {
    if let Some(message) = &error.message {
        return message.to_string();
    }
    if let (Some(min), Some(max)) = (error.params.get("min"), error.params.get("max")) {
        let label = match error.code.as_ref() {
            "range" => Some(t!("config.validate.value_should_among")),
            "length" => Some(t!("config.validate.length_should_among")),
            _ => None,
        };
        if let Some(label) = label {
            return format!("{} {}-{}", label, min, max);
        }
    }
    error.to_string()
}
