use eframe::egui;

use crate::app::App;

use super::widget::manage_button;

#[derive(Default)]
pub struct ConfigInputState {
    pub theme: String,
    pub inspect_device_activity_interval_ms: String,
    pub merge_unassociated_events_within_next_ms: String,
}

pub struct ConfigPanel {}

impl ConfigPanel {
    fn title(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let text = egui::RichText::new(text)
            .strong()
            .font(egui::epaint::FontId::proportional(15.0));
        ui.label(text)
    }

    fn config_item<R>(
        ui: &mut egui::Ui,
        text: &str,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) {
        ui.horizontal(|ui| {
            ui.label(text);
            ui.add_space(10.0);
            add_contents(ui)
        });
    }

    #[inline]
    fn textedit(text: &mut String, char_limit: usize) -> egui::TextEdit {
        egui::TextEdit::singleline(text)
            .char_limit(char_limit)
            .desired_width(char_limit as f32 * 10.0)
    }

    pub fn advanced_config(ui: &mut egui::Ui, input: &mut ConfigInputState) {
        Self::config_item(ui, "Inspect device activity internal(MS)", |ui| {
            ui.add(Self::textedit(
                &mut input.inspect_device_activity_interval_ms,
                8,
            ))
        });

        Self::config_item(ui, "Merge unassociated events within next(MS)", |ui| {
            ui.add(Self::textedit(
                &mut input.merge_unassociated_events_within_next_ms,
                8,
            ))
        });

        // For debugging colors Only
        #[cfg(debug_assertions)]
        Self::config_item(ui, "Theme(Debug):", |ui| {
            use crate::styles::Theme;
            egui::ComboBox::from_id_source("ThemeChooser")
                .selected_text(input.theme.to_string())
                .show_ui(ui, |ui| {
                    let mut add_theme = |t: Theme| {
                        ui.selectable_value(&mut input.theme, t.to_string(), t.to_string())
                    };
                    add_theme(Theme::Auto);
                    add_theme(Theme::Light);
                    add_theme(Theme::Dark);
                });
        });
    }

    const SPACING: f32 = 10.0;
    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        ui.horizontal(|ui| {
            if manage_button(ui, "Apply").clicked() {}
            if manage_button(ui, "Restore").clicked() {}
            if manage_button(ui, "Default").clicked() {}
        });

        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            Self::title(ui, "Advanced");
            ui.add_space(Self::SPACING);
            Self::advanced_config(ui, &mut app.state.config_input);
            ui.add_space(Self::SPACING);
        });
    }
}
