#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod components;
mod state;
mod styles;
mod tray;

use std::{rc::Rc, thread};

use components::about_panel::AboutPanel;
use components::config_panel::ConfigPanel;
use components::find_panel::FindPanel;
use components::manage_panel::ManagePanel;
use eframe::egui;
use log::{error, info};
use monmouse::{
    errors::Error,
    message::{setup_reactors, MasterReactor, UIPendingAction, UIReactor},
};
use state::AppState;
use styles::{gscale, Theme};
use tray::Tray;

pub fn load_icon() -> egui::IconData {
    let icon_data = include_bytes!("..\\..\\assets\\monmouse.ico");
    let image = image::load_from_memory(icon_data)
        .expect("Invalid icon data")
        .into_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let (master_reactor, _, ui_reactor) = setup_reactors();

    thread::spawn(move || {
        let eventloop = monmouse::Eventloop::new();
        let tray = Tray::new();
        match mouse_control_eventloop(eventloop, tray, &master_reactor) {
            Ok(_) => info!("mouse control eventloop exited normally"),
            Err(e) => error!("mouse control eventloop exited for error: {}", e),
        }
        master_reactor.exit_ui();
    });

    // winit wrapped by eframe, requires UI eventloop running inside main thread
    egui_eventloop(ui_reactor)
}

fn mouse_control_eventloop(
    mut eventloop: monmouse::Eventloop,
    tray: Tray,
    master_reactor: &MasterReactor,
) -> Result<(), Error> {
    eventloop.initialize()?;
    loop {
        match tray.poll_event() {
            Some(UIPendingAction::Restart) => master_reactor.restart_ui(),
            Some(UIPendingAction::Exit) => break,
            None => (),
        }
        if !eventloop.poll()? {
            break;
        }
    }
    eventloop.terminate()?;
    master_reactor.exit_ui();
    Ok(())
}

fn egui_eventloop(_ui_reactor: UIReactor) -> Result<(), eframe::Error> {
    let ui_reactor = Rc::new(_ui_reactor);

    loop {
        let ui_reactor_win = ui_reactor.clone();
        eframe::run_native(
            "MonMouse",
            ui_options_main_window(),
            Box::new(move |c| {
                App::init_ctx(&c.egui_ctx);
                Box::new(App::new(ui_reactor_win))
            }),
        )?;
        // Once clearing residual pending msg
        match ui_reactor.recv_pending_msg(false) {
            monmouse::message::UIPendingAction::Exit => break,
            monmouse::message::UIPendingAction::Restart => (),
        }
        // Actuall wait for restart msg
        match ui_reactor.recv_pending_msg(true) {
            monmouse::message::UIPendingAction::Exit => break,
            monmouse::message::UIPendingAction::Restart => (),
        }
    }
    Ok(())
}

fn ui_options_main_window() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([gscale(800.0), gscale(560.0)])
            .with_app_id("monmouse")
            .with_window_level(egui::WindowLevel::Normal)
            .with_icon(load_icon()),
        follow_system_theme: true,
        run_and_return: true,
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
    ui_reactor: Rc<UIReactor>,
}

impl App {
    fn new(ui_reactor: Rc<UIReactor>) -> Self {
        Self {
            state: AppState::default(),
            cur_panel: PanelTag::Manage,
            ui_reactor,
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
        if self.ui_reactor.check_close() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        egui::SidePanel::left("TabChooser")
            .resizable(false)
            .show_separator_line(true)
            .min_width(100.0)
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.vertical(|ui| {
                    let mut tab_button = |tag| {
                        let text = format!("{:?}", tag);
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

        // FIXME: It is a tricky way to keep triggering update(), even when the mouse is
        // outside the window area. Or else the "Exit" button in tray won't work, until
        // the mouse enter the window area.
        // Maybe by finding out a method to terminate eframe native_run outside its own eventloop.
        ctx.request_repaint();
    }
}
