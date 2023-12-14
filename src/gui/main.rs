#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod about_panel;
mod config_panel;
mod find_panel;
mod manage_panel;

mod state;
mod styles;
mod widget;

use about_panel::AboutPanel;
use config_panel::ConfigPanel;
use eframe::egui;
use find_panel::FindPanel;
use manage_panel::ManagePanel;
use state::AppState;
use styles::{gscale, Theme};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "MonMouse",
        ui_options_main_window(),
        Box::new(|c| {
            App::init_ctx(&c.egui_ctx);
            Box::<App>::default()
        }),
    )
}

fn ui_options_main_window() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([gscale(800.0), gscale(560.0)])
            .with_app_id("monmouse")
            .with_window_level(egui::WindowLevel::Normal),
        follow_system_theme: true,
        run_and_return: false,
        centered: true,
        ..Default::default()
    }
}

#[derive(PartialEq, Eq, Debug)]
enum PanelTag {
    Manage,
    Find,
    Config,
    About,
}

struct App {
    state: AppState,
    cur_panel: PanelTag,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            cur_panel: PanelTag::Manage,
        }
    }
}

impl App {
    fn init_ctx(ctx: &egui::Context) {
        ctx.set_zoom_factor(gscale(1.0));
    }

    fn update_ctx(&mut self, ctx: &egui::Context) {
        let visual = match Theme::from_string(self.state.global_config.theme.as_str()) {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        };
        ctx.set_visuals(visual);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_ctx(ctx);

        egui::SidePanel::left("TabChooser")
            .resizable(false)
            .show_separator_line(true)
            .min_width(100.0)
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.vertical(|ui| {
                    let mut tab_button = |tag| {
                        let text = format!("{:?}", tag).to_uppercase();
                        let tab = egui::RichText::from(text).heading().strong();
                        ui.selectable_value(&mut self.cur_panel, tag, tab);
                    };
                    tab_button(PanelTag::Manage);
                    tab_button(PanelTag::Find);
                    tab_button(PanelTag::Config);
                    tab_button(PanelTag::About);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.cur_panel {
                PanelTag::Manage => ManagePanel::ui(ui, &mut self.state.managed_devices),
                PanelTag::Find => FindPanel::ui(ui),
                PanelTag::Config => ConfigPanel::ui(ui, &mut self.state.global_config),
                PanelTag::About => AboutPanel::ui(ui),
            };
        });
    }
}
