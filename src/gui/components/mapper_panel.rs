use eframe::egui;
use monmouse::{
    plugin::manager::PluginManager,
    setting::{MappingRule, Settings},
};

use super::{
    validatable_input::{validation_error_text, InputField, InputsCache},
    widget::{
        error_color, form_combo, manage_button, ShortcutChoosePopup, FIELD_WIDTH,
        MANAGE_BUTTON_HEIGHT,
    },
};
use crate::app::App;

pub struct MapperPanel;

impl MapperPanel {
    fn mapper_config(
        ui: &mut egui::Ui,
        cache: &mut InputsCache<Settings>,
        plugins: &PluginManager,
    ) {
        let mut field = cache.field(|o| o.processor.mappings_field());
        let rules = field.value();
        let mut changed = false;
        ui.spacing_mut().interact_size.y = MANAGE_BUTTON_HEIGHT;
        ui.vertical(|ui| {
            let mut remove = None;
            egui::Grid::new("MapperRules")
                .num_columns(4)
                .spacing([8.0, 6.0])
                .max_col_width(FIELD_WIDTH)
                .min_row_height(MANAGE_BUTTON_HEIGHT)
                .show(ui, |ui| {
                    ui.label(t!("mapper.from"));
                    ui.label(t!("mapper.to"));
                    ui.label(t!("mapper.send_target"));
                    ui.end_row();
                    for (index, rule) in rules.iter_mut().enumerate() {
                        changed |= ShortcutChoosePopup::new(("MapperFrom", index))
                            .ui(ui, &mut rule.from)
                            .changed();
                        changed |= ShortcutChoosePopup::new(("MapperTo", index))
                            .with_mouse_buttons()
                            .ui(ui, &mut rule.to)
                            .changed();
                        let selected = match &rule.send_target {
                            None => t!("mapper.local").to_string(),
                            Some(selected) => {
                                let mut name = None;
                                plugins.for_each_send_target_option(|option| {
                                    if &option.target == selected {
                                        name = Some(option.name.clone());
                                    }
                                });
                                name.unwrap_or_else(|| {
                                    format!("{}/{}", selected.plugin, selected.callback)
                                })
                            }
                        };
                        form_combo(("SendTarget", index))
                            .width(FIELD_WIDTH)
                            .truncate()
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                changed |= ui
                                    .selectable_value(
                                        &mut rule.send_target,
                                        None,
                                        t!("mapper.local"),
                                    )
                                    .changed();
                                plugins.for_each_send_target_option(|option| {
                                    let selected =
                                        rule.send_target.as_ref() == Some(&option.target);
                                    if ui.selectable_label(selected, &option.name).clicked()
                                        && !selected
                                    {
                                        rule.send_target = Some(option.target.clone());
                                        changed = true;
                                    }
                                });
                            });
                        if ui.add(manage_button(t!("mapper.remove"))).clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                        let from_error = {
                            let field = rule.from_field();
                            field.validate(field.value).err()
                        };
                        let to_error = {
                            let field = rule.to_field();
                            field.validate(field.value).err()
                        };
                        if from_error.is_some() || to_error.is_some() {
                            for error in [from_error, to_error] {
                                if let Some(error) = error {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(validation_error_text(error))
                                                .color(error_color(ui, false)),
                                        )
                                        .wrap(),
                                    );
                                } else {
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                        }
                    }
                });
            if let Some(index) = remove {
                rules.remove(index);
                changed = true;
            }
            if ui.add(manage_button(t!("mapper.add"))).clicked() {
                rules.push(MappingRule::default());
                changed = true;
            }
        });
        let _ = field.on_field_update(changed);
    }

    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        let mut save = false;
        ui.horizontal(|ui| {
            save = ui
                .add_enabled(!app.state.saving, manage_button(t!("config.btn.Save")))
                .clicked();
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Self::mapper_config(ui, &mut app.state.mapper_cache, &app.plugins);
            });
        if save {
            app.apply_mapper_settings_async();
        }
    }
}
