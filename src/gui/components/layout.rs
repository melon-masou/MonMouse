use eframe::egui;

use crate::{
    app::App,
    components::{
        about_panel::AboutPanel,
        config_panel::ConfigPanel,
        devices_panel::DevicesPanel,
        status_bar::{status_bar_ui, status_popup_show},
    },
    PanelTag,
};

pub fn layout_ui(ctx: &egui::Context, app: &mut App) {
    egui::TopBottomPanel::bottom("StatusBar").show(ctx, |ui| {
        ui.horizontal(|ui| status_bar_ui(ui, app));
    });
    status_popup_show(ctx, app);
    egui::SidePanel::left("TabChooser")
        .resizable(false)
        .show_separator_line(true)
        .min_width(100.0)
        .show(ctx, |ui| {
            ui.add_space(5.0);
            let mut tab_button = |tag| {
                let text = format!("{:?}", tag);
                let tab = egui::RichText::from(text).heading().strong();
                if ui
                    .selectable_label(app.state.cur_panel == tag, tab)
                    .clicked()
                {
                    if let Some((_locked_panel, reason)) = &app.state.locked_panel {
                        if tag != app.state.cur_panel {
                            app.result_error_alert(reason.clone())
                        }
                    } else {
                        app.state.cur_panel = tag;
                    }
                }
            };
            tab_button(PanelTag::Devices);
            tab_button(PanelTag::Config);
            tab_button(PanelTag::About);

            #[cfg(debug_assertions)]
            app.debug_info.ui(ui);
        });
    egui::CentralPanel::default().show(ctx, |ui| {
        match app.state.cur_panel {
            PanelTag::Devices => DevicesPanel::ui(ui, app),
            PanelTag::Config => ConfigPanel::ui(ui, app),
            PanelTag::About => AboutPanel::ui(ui),
        };
    });
}
