#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod components;
mod state;
mod styles;
mod tray;

use std::{cell::RefCell, panic, process, rc::Rc, thread};

use components::about_panel::AboutPanel;
use components::config_panel::ConfigPanel;
use components::devices_panel::DevicesPanel;
use eframe::egui;
use log::{error, info};
use monmouse::{
    errors::Error,
    message::{
        setup_reactors, GenericDevice, MasterReactor, Message, MouseControlReactor, UIReactor,
    },
};
use state::{AppState, DeviceUIState};
use styles::{gscale, Theme};
use tray::{Tray, TrayEvent};

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
        .filter_level(log::LevelFilter::Info)
        .init();

    let (master_reactor, mouse_control_reactor, ui_reactor) = setup_reactors();

    set_thread_panic_process();
    thread::spawn(move || {
        let eventloop = monmouse::Eventloop::new();
        let tray = Tray::new();
        match mouse_control_eventloop(eventloop, tray, &master_reactor, &mouse_control_reactor) {
            Ok(_) => info!("mouse control eventloop exited normally"),
            Err(e) => error!("mouse control eventloop exited for error: {}", e),
        }
    });

    // winit wrapped by eframe, requires UI eventloop running inside main thread
    egui_eventloop(ui_reactor)
}

fn mouse_control_eventloop(
    mut eventloop: monmouse::Eventloop,
    tray: Tray,
    master_reactor: &MasterReactor,
    mouse_control_reactor: &MouseControlReactor,
) -> Result<(), Error> {
    eventloop.initialize()?;
    loop {
        match tray.poll_event() {
            Some(TrayEvent::Open) => master_reactor.restart_ui(),
            Some(TrayEvent::Quit) => break,
            None => (),
        }
        if !eventloop.poll()? {
            break;
        }
        eventloop.poll_message(mouse_control_reactor);
    }
    eventloop.terminate()?;
    master_reactor.exit();
    Ok(())
}

fn egui_eventloop(ui_reactor: UIReactor) -> Result<(), eframe::Error> {
    let app = Rc::new(RefCell::new(App::new(ui_reactor)));

    loop {
        let app_ref = app.clone();
        eframe::run_native(
            "MonMouse",
            ui_options_main_window(),
            Box::new(move |c| {
                AppWrap::init_ctx(&c.egui_ctx);
                Box::new(AppWrap::new(app_ref))
            }),
        )?;
        if app.borrow().ui_reactor.wait_for_restart() {
            break;
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

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
enum PanelTag {
    Devices,
    Config,
    About,
}

struct App {
    state: AppState,
    ui_reactor: UIReactor,
}

impl App {
    fn new(ui_reactor: UIReactor) -> Self {
        App {
            state: AppState::default(),
            ui_reactor,
        }
    }

    fn merge_inspect_devices(&mut self, mut devs: Vec<GenericDevice>) {
        let mut new_one = Vec::<DeviceUIState>::new();
        while let Some(v) = devs.pop() {
            new_one.push(DeviceUIState {
                locked: false,
                switch: false,
                generic: v,
            });
        }
        self.state.managed_devices = new_one;
    }

    fn dispatch_ui_msg(&mut self, ctx: &egui::Context) {
        loop {
            let msg = match self.ui_reactor.recv_msg() {
                Some(msg) => msg,
                None => return,
            };
            match msg {
                Message::Exit => {
                    self.ui_reactor.set_should_exit();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Message::CloseUI => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Message::RestartUI => drop(msg),
                Message::InspectDevices(_, result) => match result {
                    Ok(devs) => self.merge_inspect_devices(devs),
                    Err(e) => (),
                },
                Message::ApplyDevicesSetting() => todo!(),
                _ => panic!("recv unexpected ui msg: {}", msg),
            }
        }
    }
}
struct AppWrap {
    cur_panel: PanelTag,
    app: Rc<RefCell<App>>,
}

impl AppWrap {
    fn new(app: Rc<RefCell<App>>) -> Self {
        Self {
            cur_panel: PanelTag::Devices,
            app,
        }
    }
}

impl AppWrap {
    fn init_ctx(ctx: &egui::Context) {
        ctx.set_zoom_factor(gscale(1.0));
    }
}

impl eframe::App for AppWrap {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut app = self.app.borrow_mut();
        let visual = match Theme::from_string(app.state.global_config.theme.as_str()) {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        };
        ctx.set_visuals(visual);

        app.dispatch_ui_msg(ctx);

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
                    tab_button(PanelTag::Devices);
                    tab_button(PanelTag::Config);
                    tab_button(PanelTag::About);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.cur_panel {
                PanelTag::Devices => DevicesPanel::ui(ui, &mut app),
                PanelTag::Config => ConfigPanel::ui(ui, &mut app.state.global_config),
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

pub fn set_thread_panic_process() {
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        orig_hook(panic_info);
        process::exit(1);
    }));
}
