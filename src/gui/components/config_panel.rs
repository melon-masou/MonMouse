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

    fn config_item<T: ToString, P: Parser<T>>(
        ui: &mut egui::Ui,
        text: &str,
        ist: &mut InputState<T, P>,
        add_contents: impl FnOnce(&mut egui::Ui, &mut InputState<T, P>) -> bool,
    ) -> bool {
        ui.label(text);
        let changed = add_contents(ui, ist);
        if changed {
            ist.parse_only();
        }
        if let Some(errmsg) = &ist.errmsg {
            ui.label(RichText::from(errmsg.to_owned()).color(error_color(ui, false)));
        }
        ui.end_row();
        changed
    }

    #[inline]
    fn textedit(text: &mut String, char_limit: usize) -> egui::TextEdit {
        egui::TextEdit::singleline(text)
            .char_limit(char_limit)
            .desired_width(char_limit as f32 * 10.0)
    }

    pub fn advanced_config(ui: &mut egui::Ui, input: &mut ConfigInputState) {
        input.changed |= Self::config_item(
            ui,
            "Inspect device activity internal(MS)",
            &mut input.inspect_device_interval_ms,
            |ui, ist| ui.add(Self::textedit(ist.buf(), 8)).changed(),
        );

        input.changed |= Self::config_item(
            ui,
            "Merge unassociated events within next(MS)",
            &mut input.merge_unassociated_events_ms,
            |ui, ist| ui.add(Self::textedit(ist.buf(), 8)).changed(),
        );

        // For debugging colors Only
        #[cfg(debug_assertions)]
        {
            input.changed |= Self::config_item(ui, "Theme(Debug):", &mut input.theme, |ui, ist| {
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
    }

    pub fn shortcuts_config(ui: &mut egui::Ui, input: &mut ConfigInputState) {
        input.changed |= Self::config_item(
            ui,
            "Lock current mouse",
            &mut input.cur_mouse_lock,
            |ui, ist| {
                ShortcutChoosePopup::new("cur_mouse_lock")
                    .ui(ui, ist.buf())
                    .changed
            },
        );

        input.changed |= Self::config_item(
            ui,
            "Mouse jumping to next monitor",
            &mut input.cur_mouse_jump_next,
            |ui, ist| {
                ShortcutChoosePopup::new("cur_mouse_jump_next")
                    .ui(ui, ist.buf())
                    .changed
            },
        );
    }

    const SPACING: f32 = 10.0;
    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(app.state.config_input.changed, manage_button("Apply"))
                .clicked()
            {
                app.apply_new_settings();
            }
            if ui
                .add_enabled(app.state.config_input.changed, manage_button("Restore"))
                .clicked()
            {
                app.restore_settings();
                app.state.config_input.mark_changed(false);
            }
            if ui.add(manage_button("Default")).clicked() {
                app.set_default_settings();
                app.state.config_input.mark_changed(true);
            }
            if ui
                .add_enabled(!app.state.config_input.changed, manage_button("Save"))
                .clicked()
            {
                app.save_global_config();
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

struct InputState<T: ToString, P: Parser<T>> {
    buf: String,
    errmsg: Option<String>,
    p: P,
    t: std::marker::PhantomData<T>,
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
    fn set(&mut self, v: &T) {
        self.buf = v.to_string();
    }
    fn buf(&mut self) -> &mut String {
        &mut self.buf
    }
    fn parse_only(&mut self) {
        self.errmsg = self.p.parse(self.buf.as_str()).err();
    }
    fn parse_into(&mut self, dst: &mut T) -> Result<(), String> {
        self.p.parse(self.buf.as_str()).map(|v| *dst = v)
    }
}

pub struct ConfigInputState {
    changed: bool,
    theme: InputState<String, NonCheck>,
    inspect_device_interval_ms: InputState<u64, OrderParser<u64>>,
    merge_unassociated_events_ms: InputState<i64, OrderParser<i64>>,
    cur_mouse_lock: InputState<String, NonCheck>,
    cur_mouse_jump_next: InputState<String, NonCheck>,
}

impl ConfigInputState {
    pub fn mark_changed(&mut self, v: bool) {
        self.changed = v;
    }
}

impl Default for ConfigInputState {
    fn default() -> Self {
        Self {
            changed: false,
            theme: InputState::new(NonCheck()),
            inspect_device_interval_ms: InputState::new(OrderParser::new(20, 1000)),
            merge_unassociated_events_ms: InputState::new(OrderParser::new(-1, 1000)),
            cur_mouse_lock: InputState::new(NonCheck()),
            cur_mouse_jump_next: InputState::new(NonCheck()),
        }
    }
}

macro_rules! set_from {
    ($dst: expr, $src: expr, $field: ident) => {
        $dst.$field.set(&$src.$field)
    };
}
macro_rules! parse_into {
    ($dst: expr, $src: expr, $field: ident) => {
        $dst.$field.parse_into(&mut $src.$field)?
    };
}
impl ConfigInputState {
    pub fn set(&mut self, s: &Settings) {
        set_from!(self, s.ui, theme);
        set_from!(self, s.ui, inspect_device_interval_ms);
        set_from!(self, s.processor, merge_unassociated_events_ms);
        set_from!(self, s.processor.shortcuts, cur_mouse_lock);
        set_from!(self, s.processor.shortcuts, cur_mouse_jump_next);
    }

    pub fn parse_all(&mut self, s: &mut Settings) -> Result<(), String> {
        parse_into!(self, s.ui, theme);
        parse_into!(self, s.ui, inspect_device_interval_ms);
        parse_into!(self, s.processor, merge_unassociated_events_ms);
        parse_into!(self, s.processor.shortcuts, cur_mouse_lock);
        parse_into!(self, s.processor.shortcuts, cur_mouse_jump_next);
        Ok(())
    }
}
