use std::{borrow::Cow, cmp::Ordering, fmt::Display, str::FromStr};

use eframe::egui::{self, RichText, TextBuffer};
use monmouse::setting::Settings;

use crate::{app::App, font::setup_fonts_for_lang};

use super::widget::{error_color, manage_button, ShortcutChoosePopup};

pub struct ConfigPanel {}

impl ConfigPanel {
    fn title(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let text = egui::RichText::new(text)
            .strong()
            .font(egui::epaint::FontId::proportional(15.0));
        ui.label(text)
    }

    fn config_item<'a, U: FieldState>(
        ui: &mut egui::Ui,
        text: impl Into<Cow<'a, str>>,
        tooltip: Option<Cow<'a, str>>,
        ist: &mut U,
        add_contents: impl FnOnce(&mut egui::Ui, &mut U) -> bool,
    ) -> bool {
        let text = text.into();
        if let Some(tooltip) = tooltip {
            ui.horizontal(|ui| {
                ui.label(text.as_ref());
                ui.label(RichText::new("?").small().weak())
                    .on_hover_text(tooltip.as_ref());
            });
        } else {
            ui.label(text.as_ref());
        }
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

    pub fn basic_config(ui: &mut egui::Ui, input: &mut ConfigInputState) {
        let mut changed = false;

        changed |= Self::config_item(
            ui,
            t!("config.desc.language"),
            None,
            &mut input.language,
            |ui, ist| {
                egui::ComboBox::from_id_salt("LanguageChooser")
                    .selected_text(t!("lang", locale = ist.buf().as_str()))
                    .show_ui(ui, |ui| {
                        rust_i18n::available_locales!().iter().for_each(|loc| {
                            let new_lang = ui
                                .selectable_value(
                                    ist.buf(),
                                    loc.to_string(),
                                    t!("lang", locale = loc),
                                )
                                .changed();
                            if new_lang {
                                setup_fonts_for_lang(ui.ctx(), loc);
                            }
                        });
                    })
                    .response
                    .clicked()
            },
        );

        changed |= Self::config_item(
            ui,
            t!("config.desc.hide_ui_on_launch"),
            None,
            &mut input.hide_ui_on_launch,
            |ui, ist| ui.checkbox(ist.value(), "").changed(),
        );

        changed |= Self::config_item(
            ui,
            t!("config.desc.show_inactive_cursors"),
            Some(t!("config.tip.show_inactive_cursors")),
            &mut input.show_inactive_cursors,
            |ui, ist| ui.checkbox(ist.value(), "").changed(),
        );

        changed |= Self::config_item(
            ui,
            t!("config.desc.show_inactive_cursor_markers"),
            Some(t!("config.tip.show_inactive_cursor_markers")),
            &mut input.show_inactive_cursor_markers,
            |ui, ist| ui.checkbox(ist.value(), "").changed(),
        );

        input.on_changed(changed);
    }

    pub fn advanced_config(ui: &mut egui::Ui, input: &mut ConfigInputState) {
        let mut changed = false;
        changed |= Self::config_item(
            ui,
            t!("config.desc.inspect_device_interval_ms"),
            Some(t!("config.tip.inspect_device_interval_ms")),
            &mut input.inspect_device_interval_ms,
            |ui, ist| ui.add(Self::textedit(ist.buf(), 8)).changed(),
        );

        changed |= Self::config_item(
            ui,
            t!("config.desc.merge_unassociated_events_ms"),
            Some(t!("config.tip.merge_unassociated_events_ms")),
            &mut input.merge_unassociated_events_ms,
            |ui, ist| ui.add(Self::textedit(ist.buf(), 8)).changed(),
        );

        // For debugging colors Only
        #[cfg(debug_assertions)]
        {
            changed |= Self::config_item(ui, "Theme(Debug):", None, &mut input.theme, |ui, ist| {
                use crate::styles::Theme;
                egui::ComboBox::from_id_salt("ThemeChooser")
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
            t!("config.shortcut.cur_mouse_lock"),
            Some(t!("config.tip.cur_mouse_lock")),
            &mut input.cur_mouse_lock,
            |ui, ist| {
                ShortcutChoosePopup::new("cur_mouse_lock")
                    .ui(ui, ist.buf())
                    .changed
            },
        );

        changed |= Self::config_item(
            ui,
            t!("config.shortcut.cur_mouse_switch"),
            Some(t!("config.tip.cur_mouse_switch")),
            &mut input.cur_mouse_switch,
            |ui, ist| {
                ShortcutChoosePopup::new("cur_mouse_switch")
                    .ui(ui, ist.buf())
                    .changed
            },
        );

        changed |= Self::config_item(
            ui,
            t!("config.shortcut.cur_mouse_jump_next"),
            Some(t!("config.tip.cur_mouse_jump_next")),
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
                .add_enabled(
                    app.state.config_input.changed,
                    manage_button(t!("config.btn.Restore").as_str()),
                )
                .clicked()
            {
                app.restore_settings();
                app.state.config_input.on_change_restored();
                app.unlock_panel();
            }
            if ui
                .add(manage_button(t!("config.btn.Default").as_str()))
                .clicked()
            {
                app.set_default_settings();
                app.state.config_input.on_changed(true);
            }
            if ui
                .add(manage_button(t!("config.btn.Save").as_str()))
                .clicked()
            {
                app.apply_user_new_settings_async();
            }
        });

        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("BasicPart")
                .num_columns(2)
                .spacing([40.0, 8.0])
                .striped(false)
                .show(ui, |ui| {
                    Self::basic_config(ui, &mut app.state.config_input);
                });
            ui.add_space(Self::SPACING);

            Self::title(ui, t!("config.part.Shortcuts").as_str());
            ui.add_space(Self::SPACING);
            egui::Grid::new("ShortcutsPart")
                .num_columns(2)
                .spacing([40.0, 15.0])
                .striped(false)
                .show(ui, |ui| {
                    Self::shortcuts_config(ui, &mut app.state.config_input);
                });
            ui.add_space(Self::SPACING);

            Self::title(ui, t!("config.part.Advanced").as_str());
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
            app.lock_panel(t!("msg.settings_changed_cont").to_string());
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
            Err(_) => return Err(t!("config.validate.invalid_value").to_string()),
        };
        if self.min.cmp(&v) == Ordering::Greater || v.cmp(&self.max) == Ordering::Greater {
            return Err(format!(
                "{} {}-{}",
                t!("config.validate.value_should_among"),
                self.min,
                self.max
            ));
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
    language: InputState<String, NonCheck>,
    inspect_device_interval_ms: InputState<u64, OrderParser<u64>>,
    merge_unassociated_events_ms: InputState<i64, OrderParser<i64>>,
    show_inactive_cursors: ValueState<bool>,
    show_inactive_cursor_markers: ValueState<bool>,
    hide_ui_on_launch: ValueState<bool>,
    cur_mouse_lock: InputState<String, NonCheck>,
    cur_mouse_switch: InputState<String, NonCheck>,
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
            language: InputState::new(NonCheck()),
            inspect_device_interval_ms: InputState::new(OrderParser::new(20, 1000)),
            merge_unassociated_events_ms: InputState::new(OrderParser::new(-1, 1000)),
            show_inactive_cursors: ValueState::new(false),
            show_inactive_cursor_markers: ValueState::new(false),
            hide_ui_on_launch: ValueState::new(false),
            cur_mouse_lock: InputState::new(NonCheck()),
            cur_mouse_switch: InputState::new(NonCheck()),
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
        set_from!(self, s.ui, language);
        set_from!(self, s.ui, inspect_device_interval_ms);
        set_from!(self, s.ui, hide_ui_on_launch);
        set_from!(self, s.processor, merge_unassociated_events_ms);
        set_from!(self, s.processor, show_inactive_cursors);
        set_from!(self, s.processor, show_inactive_cursor_markers);
        set_from!(self, s.processor.shortcuts, cur_mouse_lock);
        set_from!(self, s.processor.shortcuts, cur_mouse_switch);
        set_from!(self, s.processor.shortcuts, cur_mouse_jump_next);
    }

    pub fn set_into(&mut self, s: &mut Settings) -> Result<(), String> {
        set_into!(self, s.ui, theme);
        set_into!(self, s.ui, language);
        set_into!(self, s.ui, inspect_device_interval_ms);
        set_into!(self, s.ui, hide_ui_on_launch);
        set_into!(self, s.processor, merge_unassociated_events_ms);
        set_into!(self, s.processor, show_inactive_cursors);
        set_into!(self, s.processor, show_inactive_cursor_markers);
        set_into!(self, s.processor.shortcuts, cur_mouse_lock);
        set_into!(self, s.processor.shortcuts, cur_mouse_switch);
        set_into!(self, s.processor.shortcuts, cur_mouse_jump_next);
        Ok(())
    }
}
