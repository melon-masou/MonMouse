#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod components;
mod config;
mod styles;
mod tray;

use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{cell::RefCell, panic, process, rc::Rc, thread};

use app::App;
use eframe::egui;
use log::info;
use monmouse::message::UINotify;
use monmouse::setting::{read_config, Settings, CONFIG_FILE_NAME};
use monmouse::{
    errors::Error,
    message::{setup_reactors, UIReactor},
};
use monmouse::{SingleProcess, POLL_MSGS, POLL_TIMEOUT};
use styles::{gscale, Theme};
use tray::Tray;

#[cfg(debug_assertions)]
use crate::components::debug::DebugInfo;
use crate::components::layout::layout_ui;
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

fn main() {
    env_logger::builder().init();
    set_thread_panic_process();
    let single_process = match SingleProcess::create() {
        Ok(v) => v,
        Err(e) => {
            exit_with_message(format!("Already launched: {}", e));
            return;
        }
    };

    let config_file = get_config_dir().map(|v| v.join(CONFIG_FILE_NAME));
    let config_path = config_file.as_ref().ok().cloned();

    let config = config_file.and_then(|v| read_config(&v));

    let egui_notify = EguiNotify::default();
    let (tray_reactor, mouse_control_reactor, ui_reactor) =
        setup_reactors(Box::new(egui_notify.clone()), Box::new(egui_notify.clone()));

    let mouse_control_thread = thread::spawn(move || {
        let eventloop = monmouse::Eventloop::new(false, mouse_control_reactor);
        let tray = Tray::new(tray_reactor);
        match mouse_control_spawn(eventloop, tray) {
            Ok(_) => info!("mouse control eventloop exited normally"),
            Err(e) => panic!("mouse control eventloop exited for error: {}", e),
        }
    });

    // winit wrapped by eframe, requires UI eventloop running inside main thread
    let result = egui_eventloop(ui_reactor, config, config_path, egui_notify);
    if let Err(e) = result {
        panic!("egui eventloop exited for: {}", e);
    }

    let _ = mouse_control_thread.join();
    drop(single_process);
}

fn mouse_control_spawn(mut eventloop: monmouse::Eventloop, tray: Tray) -> Result<(), Error> {
    eventloop.initialize()?;
    loop {
        tray.poll_events();
        if !eventloop.poll_wm_messages(POLL_MSGS, POLL_TIMEOUT)? {
            break;
        }
        if eventloop.poll_messages() {
            break;
        };
    }
    eventloop.terminate()?;
    Ok(())
}

// Without running this dummy window, the win_processor event loop may block after ui
// windows open (when hide_ui_on_launch = true). Suspect this is related to eframe
// initialization.
fn egui_dummy_launch() {
    let opts = eframe::NativeOptions {
        viewport: egui::viewport::ViewportBuilder::default()
            .with_inner_size([1.0, 1.0])
            .with_decorations(false)
            .with_visible(false)
            .with_resizable(false),
        ..Default::default()
    };

    let _ = eframe::run_simple_native("MonMouseDummy", opts, move |ctx, _frame| {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    });
}

fn egui_eventloop(
    ui_reactor: UIReactor,
    config: Result<Settings, Error>,
    config_path: Option<PathBuf>,
    egui_notify: EguiNotify,
) -> Result<(), eframe::Error> {
    let mut app = App::new(ui_reactor).load_config(config, config_path);
    app.trigger_scan_devices();
    app.trigger_system_apply_settings();

    let app = Rc::new(RefCell::new(app));
    if app.borrow_mut().on_launch_wait_start_ui(egui_dummy_launch) {
        return Ok(());
    }
    loop {
        let app_ref = app.clone();
        let egui_notify1 = egui_notify.clone();
        eframe::run_native(
            "MonMouse",
            ui_options_main_window(),
            Box::new(move |c| {
                AppWrap::init_ctx(&c.egui_ctx);
                app_ref.borrow_mut().setup_inspect_timer(&egui_notify1);
                egui_notify1.update_ctx(Some(c.egui_ctx.clone()));
                Box::new(AppWrap::new(app_ref, egui_notify1))
            }),
        )?;
        if app.borrow_mut().wait_for_restart_background() {
            break;
        }
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct EguiNotify {
    egui_ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl EguiNotify {
    pub fn update_ctx(&self, c: Option<egui::Context>) {
        *self.egui_ctx.lock().unwrap() = c;
    }
}

impl UINotify for EguiNotify {
    fn notify(&self) {
        if let Some(c) = self.egui_ctx.lock().unwrap().clone() {
            c.request_repaint()
        }
    }

    fn notify_close(&self) {
        if let Some(c) = self.egui_ctx.lock().unwrap().clone() {
            c.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
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
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone, Default)]
enum PanelTag {
    #[default]
    Devices,
    Config,
    About,
}

struct AppWrap {
    app: Rc<RefCell<App>>,
    egui_notify: EguiNotify,

    #[cfg(debug_assertions)]
    debug_info: DebugInfo,
}

impl AppWrap {
    fn new(app: Rc<RefCell<App>>, egui_notify: EguiNotify) -> Self {
        Self {
            app,
            egui_notify,

            #[cfg(debug_assertions)]
            debug_info: DebugInfo::default(),
        }
    }
}

impl AppWrap {
    fn init_ctx(ctx: &egui::Context) {
        // TODO:
        //  The value currently should be 1.0, before egui ctx.set_zoom_factor() is normal working.
        //  In case it was fixed, the value can be configurable.
        //  related issue: https://github.com/emilk/egui/issues/3736
        ctx.set_zoom_factor(1.0);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        // As a workaround, only scale fonts
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .iter_mut()
            .for_each(|font| font.1.tweak.scale = gscale(1.0));
        ctx.set_fonts(fonts);
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

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.egui_notify.update_ctx(None);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut app = self.app.borrow_mut();
        app.poll_messages();

        // Start painting
        Self::init_visuals(ctx, app.get_theme());
        layout_ui(ctx, &mut app, &mut self.debug_info);

        #[cfg(debug_assertions)]
        self.debug_info
            .on_paint(ctx.input(|input| (input.time * 1000.0).round()) as u64);
    }
}

#[cfg(target_os = "windows")]
fn exit_with_message(text: String) {
    use monmouse::windows::wintypes::WString;
    use monmouse::windows::winwrap::popup_message_box;

    let caption = WString::encode_from_str("MonMouse");
    let _ = popup_message_box(caption, WString::encode_from_str(&text));
    process::exit(1);
}

#[cfg(target_os = "windows")]
fn windows_panic_hook(panic_info: &PanicHookInfo) {
    use monmouse::windows::wintypes::WString;
    use monmouse::windows::winwrap::popup_message_box;

    let caption = WString::encode_from_str("MonMouse");
    let text = WString::encode_from_str(format!("Program panic: {}", panic_info).as_str());
    let _ = popup_message_box(caption, text);
}

fn set_thread_panic_process() {
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        orig_hook(panic_info);
        #[cfg(target_os = "windows")]
        windows_panic_hook(panic_info);
        process::exit(1);
    }));
}
