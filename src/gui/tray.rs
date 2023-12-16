use tray_icon::menu::Menu;
use tray_icon::menu::MenuEvent;
use tray_icon::menu::MenuItem;
use tray_icon::menu::PredefinedMenuItem;
use tray_icon::ClickType;
use tray_icon::TrayIcon;
use tray_icon::TrayIconBuilder;
use tray_icon::TrayIconEvent;

use crate::load_icon;

#[allow(dead_code)]
pub struct Tray {
    open: MenuItem,
    quit: MenuItem,
    trayicon: TrayIcon,
}

pub enum TrayEvent {
    Open,
    Quit,
}

impl Tray {
    pub fn new() -> Self {
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
        }
    }

    pub fn poll_event(&self) -> Option<TrayEvent> {
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if event.click_type == ClickType::Double {
                return Some(TrayEvent::Open);
            }
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.quit.id() {
                return Some(TrayEvent::Quit);
            }
            if event.id == self.open.id() {
                return Some(TrayEvent::Open);
            }
        }
        None
    }
}
