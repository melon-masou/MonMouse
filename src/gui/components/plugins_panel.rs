use eframe::egui;
use monmouse::plugin::manager::PluginEntry;

use super::{
    layout::confirm_discard,
    plugin_ui::plugin_ui,
    widget::{form_combo, manage_button},
};
use crate::app::App;

#[derive(Default)]
pub struct PluginsPanel {
    selected: usize,
    selected_tab: usize,
}

impl PluginsPanel {
    pub fn has_changes(app: &App) -> bool {
        app.plugins
            .entries
            .get(app.plugin_panel.selected)
            .is_some_and(PluginEntry::has_changes)
    }

    pub fn discard_changes(app: &mut App) {
        app.plugins.entries[app.plugin_panel.selected].discard_changes();
    }

    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        if app.plugins.entries.is_empty() {
            ui.label(t!("plugins.empty"));
            return;
        }

        if Self::plugin_controls(ui, app) {
            Self::plugin_tabs(ui, app);
        }
    }

    fn plugin_controls(ui: &mut egui::Ui, app: &mut App) -> bool {
        let selected = app.plugin_panel.selected;
        let mut next_plugin = selected;
        let entry = &app.plugins.entries[selected];
        let enabled = entry.enabled();
        let toggle = ui
            .horizontal(|ui| {
                form_combo("PluginChoice")
                    .selected_text(entry.name())
                    .show_ui(ui, |ui| {
                        for (index, entry) in app.plugins.entries.iter().enumerate() {
                            ui.selectable_value(&mut next_plugin, index, entry.name());
                        }
                    });
                ui.add_enabled(
                    entry.can_toggle(),
                    manage_button(if enabled || entry.stopping() {
                        t!("plugins.disable")
                    } else {
                        t!("plugins.enable")
                    }),
                )
                .clicked()
            })
            .inner;

        if next_plugin != selected {
            confirm_discard(app, Self::has_changes(app), move |app| {
                Self::discard_changes(app);
                app.plugin_panel.selected = next_plugin;
                app.plugin_panel.selected_tab = 0;
            });
            return false;
        }

        if toggle {
            let key = entry.key().to_owned();
            if enabled {
                confirm_discard(app, Self::has_changes(app), move |app| {
                    app.set_plugin_enabled(&key, false);
                });
            } else {
                app.set_plugin_enabled(&key, true);
            }
            return false;
        }
        enabled
    }

    fn plugin_tabs(ui: &mut egui::Ui, app: &mut App) {
        let selected = app.plugin_panel.selected;
        let entry = &mut app.plugins.entries[selected];
        let Some(plugin) = entry.plugin_mut() else {
            return;
        };
        let selected_tab = app.plugin_panel.selected_tab;

        let next_tab = Self::select_tab(ui, plugin.tabs, selected_tab);
        if next_tab != selected_tab {
            let dirty = entry.has_changes();
            confirm_discard(app, dirty, move |app| {
                Self::discard_changes(app);
                app.plugin_panel.selected_tab = next_tab;
            });
            return;
        }

        let discard_requested =
            plugin_ui(ui, plugin, selected_tab, &app.state.settings.ui.language);
        if discard_requested {
            let dirty = entry.has_changes();
            confirm_discard(app, dirty, Self::discard_changes);
        }
    }

    fn select_tab(ui: &mut egui::Ui, tabs: &[&str], selected: usize) -> usize {
        let mut next_tab = selected;
        if tabs.len() > 1 {
            ui.horizontal(|ui| {
                for (index, &tab) in tabs.iter().enumerate() {
                    ui.selectable_value(&mut next_tab, index, tab);
                }
            });
        }
        next_tab
    }
}
