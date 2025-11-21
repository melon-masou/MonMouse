use std::{cmp::Ordering, fmt::Display, str::FromStr};

use eframe::egui::{self, RichText};
use monmouse::setting::Settings;

use crate::app::App;

use super::widget::{error_color, manage_button, ShortcutChoosePopup};

pub struct ConfigPanel {}

impl ConfigPanel {
    fn title(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let text = egui::RichText::new(text)
            .strong()
            .font(egui::epaint::FontId::proportional(15.0));
        ui.label(text)
    }

    fn config_item<U: FieldState>(
        ui: &mut egui::Ui,
        text: &str,
        ist: &mut U,
        add_contents: impl FnOnce(&mut egui::Ui, &mut U) -> bool,
    ) -> bool {
        ui.label(text);
        let changed = add_contents(ui, ist);
        if changed {
            ist.parse_only();
        }
        if let Some(errmsg) = &ist.get_err() {
            ui.label(RichText::from(errmsg.to_owned()).color(error_color(ui, false)));
        }
        ui.end_row();
        changed
    }

    #[inline]
    fn textedit(text: &'_ mut String, char_limit: usize) -> egui::TextEdit<'_> {
        egui::TextEdit::singleline(text)
            .char_limit(char_limit)
            .desired_width(char_limit as f32 * 10.0)
    }

    pub fn advanced_config(ui: &mut egui::Ui, input: &mut ConfigInputState) {
        let mut changed = false;
        changed |= Self::config_item(
            ui,
            "Inspect device activity internal(MS)",
            &mut input.inspect_device_interval_ms,
            |ui, ist| ui.add(Self::textedit(ist.buf(), 8)).changed(),
        );

        changed |= Self::config_item(
            ui,
            "Merge unassociated events within next(MS)",
            &mut input.merge_unassociated_events_ms,
            |ui, ist| ui.add(Self::textedit(ist.buf(), 8)).changed(),
        );

        changed |= Self::config_item(
            ui,
            "Hide UI on launch",
            &mut input.hide_ui_on_launch,
            |ui, ist| ui.checkbox(ist.value(), "").changed(),
        );

        // For debugging colors Only
        #[cfg(debug_assertions)]
        {
            changed |= Self::config_item(ui, "Theme(Debug):", &mut input.theme, |ui, ist| {
                use crate::styles::Theme;
                egui::ComboBox::from_id_source("ThemeChooser")
                    .selected_text(ist.buf().as_str())
                    .show_ui(ui, |ui| {
                        let mut add_theme =
                            |t: Theme| ui.selectable_value(ist.buf(), t.to_string(), t.to_string());
                        add_theme(Theme::Auto).changed();
                        add_theme(Theme::Light).changed();
                        add_theme(Theme::Dark).changed();
                    })
                    .response
                    .clicked()
            });
        }
        input.on_changed(changed);
    }

    pub fn shortcuts_config(ui: &mut egui::Ui, input: &mut ConfigInputState) {
        let mut changed = false;
        changed |= Self::config_item(
            ui,
            "Lock current mouse",
            &mut input.cur_mouse_lock,
            |ui, ist| {
                ShortcutChoosePopup::new("cur_mouse_lock")
                    .ui(ui, ist.buf())
                    .changed
            },
        );

        changed |= Self::config_item(
            ui,
            "Mouse jumping to next monitor",
            &mut input.cur_mouse_jump_next,
            |ui, ist| {
                ShortcutChoosePopup::new("cur_mouse_jump_next")
                    .ui(ui, ist.buf())
                    .changed
            },
        );
        input.on_changed(changed);
    }

    const SPACING: f32 = 10.0;
    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(app.state.config_input.changed, manage_button("Restore"))
                .clicked()
            {
                app.restore_settings();
                app.state.config_input.on_change_restored();
                app.unlock_panel();
            }
            if ui.add(manage_button("Default")).clicked() {
                app.set_default_settings();
                app.state.config_input.on_changed(true);
            }
            if ui.add(manage_button("Save")).clicked() {
                app.apply_user_new_settings_async();
            }
        });

        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            Self::title(ui, "Shortcuts");
            ui.add_space(Self::SPACING);
            egui::Grid::new("ShortcutsPart")
                .num_columns(2)
                .spacing([40.0, 15.0])
                .striped(false)
                .show(ui, |ui| {
                    Self::shortcuts_config(ui, &mut app.state.config_input);
                });
            ui.add_space(Self::SPACING);

            Self::title(ui, "Advanced");
            ui.add_space(Self::SPACING);
            egui::Grid::new("AdvancedPart")
                .num_columns(2)
                .spacing([40.0, 8.0])
                .striped(false)
                .show(ui, |ui| {
                    Self::advanced_config(ui, &mut app.state.config_input);
                });
            ui.add_space(Self::SPACING);
        });

        Self::check_new_change(app);
    }

    fn check_new_change(app: &mut App) {
        if app.state.config_input.take_new_changed() {
            app.lock_panel("Settings changed but not applied".to_string());
        }
    }
}

trait Parser<T> {
    fn parse(&mut self, st: &str) -> Result<T, String>;
}

struct NonCheck();
impl Parser<String> for NonCheck {
    fn parse(&mut self, st: &str) -> Result<String, String> {
        Ok(st.to_string())
    }
}

struct OrderParser<T: Ord + FromStr + Display + Copy> {
    min: T,
    max: T,
}
impl<T: Ord + FromStr + Display + Copy> OrderParser<T> {
    fn new(min: T, max: T) -> Self {
        OrderParser { min, max }
    }
}
impl<T: Ord + FromStr + Display + Copy> Parser<T> for OrderParser<T> {
    fn parse(&mut self, st: &str) -> Result<T, String> {
        let v = match T::from_str(st) {
            Ok(v) => v,
            Err(_) => return Err("not a valid value".to_owned()),
        };
        if self.min.cmp(&v) == Ordering::Greater || v.cmp(&self.max) == Ordering::Greater {
            return Err(format!("value should among {}-{}", self.min, self.max));
        }
        Ok(v)
    }
}

trait FieldState {
    fn parse_only(&mut self);
    fn get_err(&self) -> Option<&str>;
}

struct ValueState<T: Copy> {
    v: T,
}

impl<T: Copy> FieldState for ValueState<T> {
    fn parse_only(&mut self) {}

    fn get_err(&self) -> Option<&str> {
        None
    }
}

impl<T: Copy> ValueState<T> {
    fn new(v: T) -> Self {
        Self { v }
    }
    fn value(&mut self) -> &mut T {
        &mut self.v
    }
    fn set_from(&mut self, v: &T) {
        self.v = *v;
    }
    fn set_into(&mut self, dst: &mut T) -> Result<(), String> {
        *dst = self.v;
        Ok(())
    }
}

struct InputState<T: ToString, P: Parser<T>> {
    buf: String,
    errmsg: Option<String>,
    p: P,
    t: std::marker::PhantomData<T>,
}

impl<T: ToString, P: Parser<T>> FieldState for InputState<T, P> {
    fn parse_only(&mut self) {
        self.errmsg = self.p.parse(self.buf.as_str()).err();
    }
    fn get_err(&self) -> Option<&str> {
        self.errmsg.as_deref()
    }
}

impl<T: ToString, P: Parser<T>> InputState<T, P> {
    fn new(p: P) -> Self {
        Self {
            buf: String::default(),
            errmsg: None,
            p,
            t: std::marker::PhantomData,
        }
    }
    fn buf(&mut self) -> &mut String {
        &mut self.buf
    }
    fn set_from(&mut self, v: &T) {
        self.buf = v.to_string();
    }
    fn set_into(&mut self, dst: &mut T) -> Result<(), String> {
        self.p.parse(self.buf.as_str()).map(|v| *dst = v)
    }
}

pub struct ConfigInputState {
    changed: bool,
    have_new_change: bool,
    theme: InputState<String, NonCheck>,
    inspect_device_interval_ms: InputState<u64, OrderParser<u64>>,
    merge_unassociated_events_ms: InputState<i64, OrderParser<i64>>,
    hide_ui_on_launch: ValueState<bool>,
    cur_mouse_lock: InputState<String, NonCheck>,
    cur_mouse_jump_next: InputState<String, NonCheck>,
}

impl ConfigInputState {
    pub fn on_changed(&mut self, changed: bool) {
        if changed && !self.changed {
            self.changed = true;
            self.have_new_change = true;
        }
    }
    pub fn on_change_applied(&mut self) {
        self.changed = false;
    }
    pub fn on_change_restored(&mut self) {
        self.changed = false;
    }
    pub fn take_new_changed(&mut self) -> bool {
        if self.have_new_change {
            self.have_new_change = false;
            return true;
        }
        false
    }
}

impl Default for ConfigInputState {
    fn default() -> Self {
        Self {
            changed: false,
            have_new_change: false,
            theme: InputState::new(NonCheck()),
            inspect_device_interval_ms: InputState::new(OrderParser::new(20, 1000)),
            merge_unassociated_events_ms: InputState::new(OrderParser::new(-1, 1000)),
            hide_ui_on_launch: ValueState::new(false),
            cur_mouse_lock: InputState::new(NonCheck()),
            cur_mouse_jump_next: InputState::new(NonCheck()),
        }
    }
}

macro_rules! set_from {
    ($dst: expr, $src: expr, $field: ident) => {
        $dst.$field.set_from(&$src.$field)
    };
}
macro_rules! set_into {
    ($dst: expr, $src: expr, $field: ident) => {
        $dst.$field.set_into(&mut $src.$field)?
    };
}
impl ConfigInputState {
    pub fn set_from(&mut self, s: &Settings) {
        set_from!(self, s.ui, theme);
        set_from!(self, s.ui, inspect_device_interval_ms);
        set_from!(self, s.ui, hide_ui_on_launch);
        set_from!(self, s.processor, merge_unassociated_events_ms);
        set_from!(self, s.processor.shortcuts, cur_mouse_lock);
        set_from!(self, s.processor.shortcuts, cur_mouse_jump_next);
    }

    pub fn set_into(&mut self, s: &mut Settings) -> Result<(), String> {
        set_into!(self, s.ui, theme);
        set_into!(self, s.ui, inspect_device_interval_ms);
        set_into!(self, s.ui, hide_ui_on_launch);
        set_into!(self, s.processor, merge_unassociated_events_ms);
        set_into!(self, s.processor.shortcuts, cur_mouse_lock);
        set_into!(self, s.processor.shortcuts, cur_mouse_jump_next);
        Ok(())
    }
}
