use eframe::egui;

use crate::App;

use super::widget::{error_color, indicator_ui};

pub fn status_bar_ui(ui: &mut egui::Ui, app: &mut App) {
    let mut clear_err = false;

    if let Some(err_msg) = &app.last_error {
        if ui
            .add(egui::Button::new("❌").frame(false))
            .on_hover_text("Ignore")
            .clicked()
        {
            clear_err = true;
        }
        if ui
            .add(egui::Button::new("📋").frame(false))
            .on_hover_text("Copy")
            .clicked()
        {
            ui.output_mut(|o| {
                o.copied_text = err_msg.clone();
            });
        }
        indicator_ui(ui, error_color(ui, false));
        ui.label(err_msg.as_str()).on_hover_text(err_msg.as_str());
    }

    if clear_err {
        app.last_error = None;
    }
}
