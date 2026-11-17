use eframe::egui::{self, TextBuffer};
use monmouse::setting::Settings;

use crate::{app::App, font::setup_fonts_for_lang};

use super::{
    validatable_input::{InputItem, InputsCache},
    widget::{
        form_combo, manage_button, number_input, section_title, ShortcutChoosePopup, FORM_SPACING,
        SECTION_SPACING,
    },
};

pub struct ConfigPanel {}

impl ConfigPanel {
    pub fn basic_config(ui: &mut egui::Ui, cache: &mut InputsCache<Settings>) {
        InputItem::new(
            t!("config.desc.language"),
            cache.field(|o| o.ui.language_field()),
        )
        .show(ui, |ui, language| {
            let mut changed = false;
            form_combo("LanguageChooser")
                .selected_text(t!("lang", locale = language.as_str()))
                .show_ui(ui, |ui| {
                    for loc in rust_i18n::available_locales!() {
                        let new_lang = ui
                            .selectable_value(language, loc.to_string(), t!("lang", locale = loc))
                            .changed();
                        if new_lang {
                            setup_fonts_for_lang(ui.ctx(), &loc);
                        }
                        changed |= new_lang;
                    }
                });
            changed
        });

        InputItem::new(
            t!("config.desc.hide_ui_on_launch"),
            cache.field(|o| o.ui.hide_ui_on_launch_field()),
        )
        .show(ui, |ui, value| ui.checkbox(value, "").changed());

        InputItem::new(
            t!("config.desc.show_inactive_cursors"),
            cache.field(|o| o.processor.show_inactive_cursors_field()),
        )
        .with_tip(t!("config.tip.show_inactive_cursors"))
        .show(ui, |ui, value| ui.checkbox(value, "").changed());

        InputItem::new(
            t!("config.desc.show_inactive_cursor_markers"),
            cache.field(|o| o.processor.show_inactive_cursor_markers_field()),
        )
        .with_tip(t!("config.tip.show_inactive_cursor_markers"))
        .show(ui, |ui, value| ui.checkbox(value, "").changed());
    }

    pub fn advanced_config(ui: &mut egui::Ui, cache: &mut InputsCache<Settings>) {
        InputItem::new(
            t!("config.desc.inspect_device_interval_ms"),
            cache.text_field(|o| o.ui.inspect_device_interval_ms_field()),
        )
        .with_tip(t!("config.tip.inspect_device_interval_ms"))
        .show(ui, |ui, text| ui.add(number_input(text)).changed());

        InputItem::new(
            t!("config.desc.merge_unassociated_events_ms"),
            cache.text_field(|o| o.processor.merge_unassociated_events_ms_field()),
        )
        .with_tip(t!("config.tip.merge_unassociated_events_ms"))
        .show(ui, |ui, text| ui.add(number_input(text)).changed());

        // For debugging colors Only
        #[cfg(debug_assertions)]
        {
            InputItem::new("Theme(Debug):", cache.field(|o| o.ui.theme_field())).show(
                ui,
                |ui, theme| {
                    use crate::styles::Theme;
                    let mut changed = false;
                    form_combo("ThemeChooser")
                        .selected_text(theme.as_str())
                        .show_ui(ui, |ui| {
                            for option in [Theme::Auto, Theme::Light, Theme::Dark] {
                                changed |= ui
                                    .selectable_value(theme, option.to_string(), option.to_string())
                                    .changed();
                            }
                        });
                    changed
                },
            );
        }
    }

    pub fn shortcuts_config(ui: &mut egui::Ui, cache: &mut InputsCache<Settings>) {
        InputItem::new(
            t!("config.shortcut.cur_mouse_lock"),
            cache.field(|o| o.processor.shortcuts.cur_mouse_lock_field()),
        )
        .with_tip(t!("config.tip.cur_mouse_lock"))
        .show(ui, |ui, shortcut| {
            ShortcutChoosePopup::new("cur_mouse_lock")
                .ui(ui, shortcut)
                .changed()
        });

        InputItem::new(
            t!("config.shortcut.cur_mouse_switch"),
            cache.field(|o| o.processor.shortcuts.cur_mouse_switch_field()),
        )
        .with_tip(t!("config.tip.cur_mouse_switch"))
        .show(ui, |ui, shortcut| {
            ShortcutChoosePopup::new("cur_mouse_switch")
                .ui(ui, shortcut)
                .changed()
        });

        InputItem::new(
            t!("config.shortcut.cur_mouse_jump_next"),
            cache.field(|o| o.processor.shortcuts.cur_mouse_jump_next_field()),
        )
        .with_tip(t!("config.tip.cur_mouse_jump_next"))
        .show(ui, |ui, shortcut| {
            ShortcutChoosePopup::new("cur_mouse_jump_next")
                .ui(ui, shortcut)
                .changed()
        });
    }

    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        let mut save = false;
        ui.horizontal(|ui| {
            if ui
                .add(manage_button(t!("config.btn.Default").as_str()))
                .clicked()
            {
                app.set_default_settings();
            }
            save = ui
                .add_enabled(
                    !app.state.saving,
                    manage_button(t!("config.btn.Save").as_str()),
                )
                .clicked();
        });

        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("BasicPart")
                    .num_columns(2)
                    .spacing(FORM_SPACING)
                    .striped(false)
                    .show(ui, |ui| {
                        Self::basic_config(ui, &mut app.state.config_cache);
                    });
                ui.add_space(SECTION_SPACING);

                section_title(ui, t!("config.part.Shortcuts").as_str());
                ui.add_space(SECTION_SPACING);
                egui::Grid::new("ShortcutsPart")
                    .num_columns(2)
                    .spacing(FORM_SPACING)
                    .striped(false)
                    .show(ui, |ui| {
                        Self::shortcuts_config(ui, &mut app.state.config_cache);
                    });
                ui.add_space(SECTION_SPACING);

                section_title(ui, t!("config.part.Advanced").as_str());
                ui.add_space(SECTION_SPACING);
                egui::Grid::new("AdvancedPart")
                    .num_columns(2)
                    .spacing(FORM_SPACING)
                    .striped(false)
                    .show(ui, |ui| {
                        ui.label(t!("config.desc.config_file"));
                        if ui
                            .add(manage_button(t!("config.btn.Locate").as_str()))
                            .on_hover_text(t!("config.tip.locate_config_file"))
                            .clicked()
                        {
                            app.locate_config_file();
                        }
                        ui.end_row();
                        Self::advanced_config(ui, &mut app.state.config_cache);
                    });
                ui.add_space(SECTION_SPACING);
            });

        if save {
            app.apply_user_new_settings_async();
        }
    }
}
