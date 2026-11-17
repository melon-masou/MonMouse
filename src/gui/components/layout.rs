use eframe::egui;
use rust_i18n::t;

use crate::{
    app::App,
    components::{
        about_panel::AboutPanel,
        config_panel::ConfigPanel,
        devices_panel::DevicesPanel,
        status_bar::{status_bar_ui, status_popup_show},
        validatable_input::InputsCache,
        widget::{manage_button, NotificationPopup, SECTION_SPACING},
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
                let text = t!(format!("panel.{:?}", tag));
                let tab = egui::RichText::from(text).heading().strong();
                if ui
                    .selectable_label(app.state.cur_panel == tag, tab)
                    .clicked()
                {
                    switch_panel(app, tag);
                }
            };
            tab_button(PanelTag::Devices);
            tab_button(PanelTag::Config);
            tab_button(PanelTag::About);

            #[cfg(debug_assertions)]
            app.debug_info.ui(ui);
        });
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(SECTION_SPACING as i8, 0))
            .show(ui, |ui| match app.state.cur_panel {
                PanelTag::Devices => DevicesPanel::ui(ui, app),
                PanelTag::Config => ConfigPanel::ui(ui, app),
                PanelTag::About => AboutPanel::ui(ui),
            });
    });
    show_discard_confirmation(ctx, app);
}

fn switch_panel(app: &mut App, tag: PanelTag) {
    if tag == app.state.cur_panel {
        return;
    }
    let dirty = match app.state.cur_panel {
        PanelTag::Config => app.state.config_cache.has_changes(),
        _ => false,
    };
    confirm_discard(app, dirty, move |app| {
        if app.state.cur_panel == PanelTag::Config {
            app.state.config_cache = InputsCache::new(app.state.settings.clone());
        }
        app.state.cur_panel = tag;
    });
}

pub fn confirm_discard(app: &mut App, dirty: bool, action: impl FnOnce(&mut App) + 'static) {
    if dirty {
        app.state.discard_action = Some(Box::new(action));
    } else {
        action(app);
    }
}

fn show_discard_confirmation(ctx: &egui::Context, app: &mut App) {
    if app.state.discard_action.is_none() {
        return;
    }
    let mut popup = NotificationPopup::new("discard_settings_changes");
    popup.margin = egui::Margin::same(popup.margin.left);
    ctx.memory_mut(|memory| {
        memory.set_modal_layer(egui::LayerId::new(egui::Order::Foreground, popup.id));
    });
    let response = popup.show_with_actions(
        ctx,
        egui::RichText::new(t!("msg.discard_changes")).strong(),
        |ui, _| {
            ui.set_width(popup.max_width);
            let (discard, cancel) = ui.columns(2, |columns| {
                let discard = columns[0]
                    .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(manage_button(t!("btn.Discard"))).clicked()
                    })
                    .inner;
                let cancel = columns[1]
                    .with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(manage_button(t!("btn.Cancel"))).clicked()
                    })
                    .inner;
                (discard, cancel)
            });
            if cancel {
                Some(false)
            } else if discard {
                Some(true)
            } else {
                None
            }
        },
    );
    let discard = if response.action.will_close() {
        Some(false)
    } else {
        response.inner
    };
    if let Some(discard) = discard {
        let action = app.state.discard_action.take().unwrap();
        if discard {
            action(app);
        }
    }
}
