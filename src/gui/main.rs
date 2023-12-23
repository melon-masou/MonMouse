#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod components;
mod config;
mod styles;
mod tray;

use std::panic::PanicInfo;
use std::path::PathBuf;
use std::{cell::RefCell, panic, process, rc::Rc, thread};

use app::App;
use components::about_panel::AboutPanel;
use components::config_panel::ConfigPanel;
use components::devices_panel::DevicesPanel;
use components::status_bar::{status_bar_ui, status_popup_show};
use eframe::egui;
use log::info;
use monmouse::setting::{read_config, Settings, CONFIG_FILE_NAME};
use monmouse::{
    errors::Error,
    message::{setup_reactors, MasterReactor, UIReactor},
};
use monmouse::{POLL_MSGS, POLL_TIMEOUT};
use styles::{gscale, Theme};
use tray::{Tray, TrayEvent};

#[cfg(debug_assertions)]
use crate::components::debug::DebugInfo;
use crate::config::get_config_dir;

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
    env_logger::builder().init();

    let config_file = get_config_dir().map(|v| v.join(CONFIG_FILE_NAME));
    let config_path = config_file.as_ref().ok().cloned();

    let config = config_file.and_then(|v| read_config(&v));

    let (master_reactor, mouse_control_reactor, ui_reactor) = setup_reactors();

    set_thread_panic_process();
    let mouse_control_thread = thread::spawn(move || {
        let eventloop = monmouse::Eventloop::new(false, mouse_control_reactor);
        let tray = Tray::new();
        match mouse_control_eventloop(eventloop, tray, &master_reactor) {
            Ok(_) => info!("mouse control eventloop exited normally"),
            Err(e) => panic!("mouse control eventloop exited for error: {}", e),
        }
    });

    // winit wrapped by eframe, requires UI eventloop running inside main thread
    let result = egui_eventloop(ui_reactor, config, config_path);
    let _ = mouse_control_thread.join();
    result
}

fn mouse_control_eventloop(
    mut eventloop: monmouse::Eventloop,
    tray: Tray,
    master_reactor: &MasterReactor,
) -> Result<(), Error> {
    eventloop.initialize()?;
    loop {
        match tray.poll_event() {
            Some(TrayEvent::Open) => master_reactor.restart_ui(),
            Some(TrayEvent::Quit) => break,
            None => (),
        }
        if !eventloop.poll(POLL_MSGS, POLL_TIMEOUT)? {
            break;
        }
        eventloop.poll_message();
    }
    eventloop.terminate()?;
    master_reactor.exit();
    Ok(())
}

fn egui_eventloop(
    ui_reactor: UIReactor,
    config: Result<Settings, Error>,
    config_path: Option<PathBuf>,
) -> Result<(), eframe::Error> {
    let mut app = App::new(ui_reactor).load_config(config, config_path);
    app.trigger_scan_devices();
    app.trigger_settings_changed();

    let app = Rc::new(RefCell::new(app));
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
        if app.borrow().wait_for_restart() {
            break;
        }
    }
    Ok(())
}

fn ui_options_main_window() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([gscale(800.0), gscale(400.0)])
            .with_app_id("monmouse")
            .with_window_level(egui::WindowLevel::Normal)
            .with_icon(load_icon()),
        follow_system_theme: true,
        run_and_return: true,
        centered: true,
        persist_window: true,
        ..Default::default()
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
enum PanelTag {
    Devices,
    Config,
    About,
}

struct AppWrap {
    cur_panel: PanelTag,
    app: Rc<RefCell<App>>,
    #[cfg(debug_assertions)]
    debug_info: DebugInfo,
}

impl AppWrap {
    fn new(app: Rc<RefCell<App>>) -> Self {
        Self {
            cur_panel: PanelTag::Devices,
            app,
            #[cfg(debug_assertions)]
            debug_info: DebugInfo::default(),
        }
    }
}

impl AppWrap {
    fn init_ctx(ctx: &egui::Context) {
        ctx.set_zoom_factor(gscale(1.0));
    }

    fn init_visuals(ctx: &egui::Context, theme: Theme) {
        match theme {
            Theme::Light => ctx.set_visuals(egui::Visuals::light()),
            Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
            Theme::Auto => (),
        };
    }
}

impl eframe::App for AppWrap {
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut app = self.app.borrow_mut();
        Self::init_visuals(ctx, app.get_theme());

        egui::TopBottomPanel::bottom("StatusBar").show(ctx, |ui| {
            ui.horizontal(|ui| status_bar_ui(ui, &mut app));
        });

        #[cfg(debug_assertions)]
        let debug_info = &self.debug_info;
        egui::SidePanel::left("TabChooser")
            .resizable(false)
            .show_separator_line(true)
            .min_width(100.0)
            .show(ctx, |ui| {
                ui.add_space(5.0);
                let mut tab_button = |tag| {
                    let text = format!("{:?}", tag);
                    let tab = egui::RichText::from(text).heading().strong();
                    ui.selectable_value(&mut self.cur_panel, tag, tab);
                };
                tab_button(PanelTag::Devices);
                tab_button(PanelTag::Config);
                tab_button(PanelTag::About);

                #[cfg(debug_assertions)]
                debug_info.ui(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.cur_panel {
                PanelTag::Devices => DevicesPanel::ui(ui, &mut app),
                PanelTag::Config => ConfigPanel::ui(ui, &mut app),
                PanelTag::About => AboutPanel::ui(ui),
            };
        });

        status_popup_show(ctx, &mut app);

        let tick_ms = ctx.input(|input| (input.time * 1000.0).round()) as u64;

        #[cfg(debug_assertions)]
        self.debug_info.on_paint(tick_ms);

        // FIXME: It is a tricky way to keep triggering update(), even when the mouse is
        // outside the window area. Or else the "Exit" button in tray won't work, until
        // the mouse enter the window area.
        // Maybe by finding out a method to terminate eframe native_run outside its own eventloop.
        ctx.request_repaint();
        // Following eventloop, should be also placed there
        app.trigger_inspect_devices_status(tick_ms);
        app.dispatch_ui_msg(ctx);
    }
}

#[cfg(target_os = "windows")]
pub fn windows_panic_hook(panic_info: &PanicInfo) {
    use monmouse::windows::wintypes::WString;
    use monmouse::windows::winwrap::popup_message_box;

    let caption = WString::encode_from_str("MonMouse");
    let text = WString::encode_from_str(format!("Program panic: {}", panic_info).as_str());
    let _ = popup_message_box(caption, text);
}

pub fn set_thread_panic_process() {
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        orig_hook(panic_info);
        #[cfg(target_os = "windows")]
        windows_panic_hook(panic_info);
        process::exit(1);
    }));
}
