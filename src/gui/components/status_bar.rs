use eframe::egui;

use crate::app::{App, StatusBarResult};

use super::widget::{error_color, indicator_ui, NotificationPopup};

pub fn status_bar_ui(ui: &mut egui::Ui, app: &mut App) {
    let msg_with_bottons = |ui: &mut egui::Ui, ok: bool, msg: &String| {
        #[cfg(debug_assertions)]
        if ui
            .add(egui::Button::new("📋").frame(false))
            .on_hover_text("Copy")
            .clicked()
        {
            ui.output_mut(|o| {
                o.copied_text = msg.clone();
            });
        }
        indicator_ui(ui, error_color(ui, ok));
        ui.label(msg.as_str()).on_hover_text(msg.as_str());
    };

    match &app.last_result {
        StatusBarResult::Ok(msg) => {
            msg_with_bottons(ui, true, msg);
        }
        StatusBarResult::ErrMsg(msg) => {
            msg_with_bottons(ui, false, msg);
        }
        StatusBarResult::None => (),
    };
}

pub fn status_popup_show(ctx: &egui::Context, app: &mut App) {
    if !app.alert_errors.is_empty() {
        let rsp = NotificationPopup::new("StatusNotificationPopup").show(ctx, "Errors", |ui, _| {
            for err in &app.alert_errors {
                ui.label(err);
            }
        });
        if rsp.action.will_close() {
            app.alert_errors.clear();
        }
    }
}
