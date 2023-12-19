use eframe::egui;

use crate::app::GlobalConfig;

pub struct ConfigPanel {}

impl ConfigPanel {
    pub fn ui(ui: &mut egui::Ui, config: &mut GlobalConfig) {
        ui.vertical(|ui| {
            // For debugging colors Only
            #[cfg(debug_assertions)]
            ui.horizontal(|ui| {
                use crate::styles::Theme;
                ui.label("Theme(Debug): ");
                egui::ComboBox::from_id_source("ThemeChooser")
                    .selected_text(config.theme.to_string())
                    .show_ui(ui, |ui| {
                        let mut add_theme = |t: Theme| {
                            ui.selectable_value(&mut config.theme, t.to_string(), t.to_string())
                        };
                        add_theme(Theme::Auto);
                        add_theme(Theme::Light);
                        add_theme(Theme::Dark);
                    });
            });
        });
    }
}
