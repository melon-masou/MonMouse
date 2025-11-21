use monmouse::message::TrayReactor;
use tray_icon::menu::Menu;
use tray_icon::menu::MenuEvent;
use tray_icon::menu::MenuItem;
use tray_icon::menu::PredefinedMenuItem;
use tray_icon::MouseButton;
use tray_icon::TrayIcon;
use tray_icon::TrayIconBuilder;
use tray_icon::TrayIconEvent;

use crate::load_icon;

#[allow(dead_code)]
pub struct Tray {
    open: MenuItem,
    quit: MenuItem,
    trayicon: TrayIcon,
    tray_reactor: TrayReactor,
}

impl Tray {
    pub fn new(tray_reactor: TrayReactor) -> Self {
        let icon = load_icon();
        let tray_menu = Menu::new();

        let open = MenuItem::new("Open", true, None);
        let quit = MenuItem::new("Quit", true, None);

        tray_menu
            .append_items(&[&open, &PredefinedMenuItem::separator(), &quit])
            .unwrap();

        let trayicon = TrayIconBuilder::new()
            .with_tooltip("MonMouse")
            .with_menu(Box::new(tray_menu))
            .with_menu_on_left_click(false)
            .with_icon(
                tray_icon::Icon::from_rgba(icon.rgba, icon.width, icon.height)
                    .expect("Failed to open icon"),
            )
            .build()
            .unwrap();
        Self {
            open,
            quit,
            trayicon,
            tray_reactor,
        }
    }

    pub fn poll_events(&self) {
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                self.tray_reactor.restart_ui();
            }
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit.id() {
                self.tray_reactor.exit();
            }
            if event.id == self.open.id() {
                self.tray_reactor.restart_ui();
            }
        }
    }
}
