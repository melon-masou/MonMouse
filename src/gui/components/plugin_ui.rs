use eframe::egui;
use monmouse::plugin::manager::Plugin;
use monmouse_lib::plugin::{Draw, Input, Layout, PluginUi, StatusLight, UiBackend};

use super::{
    validatable_input::input_label,
    widget::{
        error_color, form_combo, icon_button, indicator_ui, manage_button, section_title,
        text_input, ShortcutChoosePopup, FIELD_WIDTH, FORM_SPACING, MANAGE_BUTTON_HEIGHT,
        NUMBER_WIDTH, SECTION_SPACING,
    },
};

struct Context<'a> {
    language: &'a str,
    discard: bool,
}

pub fn plugin_ui(ui: &mut egui::Ui, plugin: &mut Plugin, tab: usize, language: &str) -> bool {
    let mut context = Context {
        language,
        discard: false,
    };
    ui.push_id((plugin.key, tab), |ui| {
        let mut backend = EguiPluginUiBackend {
            ui,
            context: &mut context,
        };
        plugin.ui(tab, &mut PluginUi::new(&mut backend));
    });
    context.discard
}

struct EguiPluginUiBackend<'a, 'b> {
    ui: &'a mut egui::Ui,
    context: &'a mut Context<'b>,
}

fn draw(ui: &mut egui::Ui, context: &mut Context<'_>, body: &mut Draw<'_>) {
    body(&mut PluginUi::new(&mut EguiPluginUiBackend { ui, context }));
}

impl UiBackend for EguiPluginUiBackend<'_, '_> {
    fn language(&self) -> &str {
        self.context.language
    }

    fn layout(&mut self, layout: Layout<'_>, body: &mut Draw<'_>) {
        let context = &mut *self.context;
        let ui = &mut *self.ui;
        match layout {
            Layout::Scope(key) => {
                let id = ui.make_persistent_id(key);
                ui.scope_builder(egui::UiBuilder::new().id(id), |ui| draw(ui, context, body));
            }
            Layout::Toolbar => {
                ui.horizontal(|ui| draw(ui, context, body));
                ui.separator();
            }
            Layout::Row => {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), MANAGE_BUTTON_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| draw(ui, context, body),
                );
            }
            Layout::Card => {
                egui::Frame::group(ui.style())
                    .inner_margin(SECTION_SPACING)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        draw(ui, context, body);
                    });
                ui.add_space(SECTION_SPACING);
            }
            Layout::Form { enabled } => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if !enabled {
                            ui.disable();
                        }
                        draw(ui, context, body);
                    });
            }
            Layout::Section { title } => {
                if !title.is_empty() {
                    section_title(ui, title);
                }
                let id = ui.next_auto_id();
                egui::Grid::new(id)
                    .num_columns(2)
                    .spacing(FORM_SPACING)
                    .show(ui, |ui| draw(ui, context, body));
                ui.add_space(SECTION_SPACING);
            }
            Layout::Field { label, hint } => {
                input_label(ui, label, hint);
                let id = ui.next_auto_id();
                ui.push_id(id, |ui| {
                    ui.horizontal(|ui| draw(ui, context, body));
                });
                ui.end_row();
            }
        }
    }

    fn collapsible_card(
        &mut self,
        title: &str,
        expanded: bool,
        actions: &mut Draw<'_>,
        body: &mut Draw<'_>,
    ) -> bool {
        let context = &mut *self.context;
        egui::Frame::group(self.ui.style())
            .inner_margin(SECTION_SPACING)
            .show(self.ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = FORM_SPACING[1];
                let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    ui.next_auto_id(),
                    expanded,
                );
                state.set_open(expanded);
                let (toggle, header, _) = state
                    .show_header(ui, |ui| {
                        let clicked = section_title(ui, title)
                            .interact(egui::Sense::click())
                            .clicked();
                        draw(ui, context, actions);
                        clicked
                    })
                    .body_unindented(|ui| {
                        if expanded {
                            draw(ui, context, body);
                        }
                    });
                toggle.clicked() || header.inner
            })
            .inner
    }

    fn heading(&mut self, text: &str) {
        section_title(self.ui, text);
    }

    fn text(&mut self, text: &str) {
        self.ui.label(text);
    }

    fn status(&mut self, label: &str, light: StatusLight, detail: &str) {
        self.ui
            .horizontal(|ui| {
                let color = match light {
                    StatusLight::Off => ui.style().visuals.widgets.inactive.bg_fill,
                    StatusLight::On => error_color(ui, true),
                    StatusLight::Error => error_color(ui, false),
                };
                indicator_ui(ui, color);
                ui.label(label);
            })
            .response
            .on_hover_text(detail);
    }

    fn icon_button(&mut self, icon: &str, hint: &str) -> bool {
        self.ui
            .add(icon_button(self.ui, icon))
            .on_hover_text(hint)
            .clicked()
    }

    fn edit(&mut self, input: Input<'_>, width_chars: Option<usize>) -> bool {
        let ui = &mut *self.ui;
        let width = width_chars.map(|chars| {
            let font = egui::TextStyle::Body.resolve(ui.style());
            ui.fonts_mut(|fonts| fonts.glyph_width(&font, '0')) * chars as f32
        });
        match input {
            Input::Text {
                value,
                max_chars,
                placeholder,
            } => {
                let width = width.map_or(FIELD_WIDTH, |width| width + 8.0);
                let changed = ui
                    .add_sized(
                        [width, ui.spacing().interact_size.y],
                        text_input(value, max_chars.unwrap_or(usize::MAX))
                            .desired_width(width)
                            .hint_text(placeholder),
                    )
                    .changed();
                if let Some(max) = max_chars.filter(|max| value.chars().count() > *max) {
                    ui.colored_label(
                        error_color(ui, false),
                        format!("{} 0–{max}", t!("config.validate.length_should_among")),
                    );
                }
                changed
            }
            Input::Unsigned { value, range } => {
                let changed = ui
                    .add_sized(
                        [width.unwrap_or(NUMBER_WIDTH), ui.spacing().interact_size.y],
                        egui::DragValue::new(value)
                            .range(range.clone())
                            .clamp_existing_to_range(false),
                    )
                    .changed();
                if !range.contains(value) {
                    ui.colored_label(
                        error_color(ui, false),
                        format!(
                            "{} {}–{}",
                            t!("config.validate.value_should_among"),
                            range.start(),
                            range.end()
                        ),
                    );
                }
                changed
            }
            Input::Bool(value) => ui.checkbox(value, "").changed(),
            Input::Choice { selected, labels } => {
                let enabled = ui.is_enabled();
                let previous = *selected;
                form_combo(ui.next_auto_id())
                    .width(width.unwrap_or(FIELD_WIDTH))
                    .selected_text(labels.get(previous).copied().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        if !enabled {
                            ui.disable();
                        }
                        for (index, label) in labels.iter().enumerate() {
                            ui.selectable_value(selected, index, *label);
                        }
                    });
                previous != *selected
            }
            Input::Shortcut(value) => ShortcutChoosePopup::new(ui.next_auto_id())
                .with_width(width.unwrap_or(FIELD_WIDTH))
                .ui(ui, value)
                .changed(),
        }
    }

    fn button(&mut self, label: &str, enabled: bool) -> bool {
        self.ui.add_enabled(enabled, manage_button(label)).clicked()
    }

    fn request_discard(&mut self) {
        self.context.discard = true;
    }
}
