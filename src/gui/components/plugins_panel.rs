use eframe::egui;

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
            .is_some_and(|plugin| plugin.enabled() && plugin.main.has_changes())
    }

    pub fn discard_changes(app: &mut App) {
        if let Some(plugin) = app.plugins.entries.get_mut(app.plugin_panel.selected) {
            if plugin.main.has_changes() {
                plugin.main.discard_changes();
            }
        }
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
        let plugin = &app.plugins.entries[selected];
        let key = plugin.key;
        let enabled = plugin.enabled();
        let stopping = plugin.stopping();
        let toggle = ui
            .horizontal(|ui| {
                form_combo("PluginChoice")
                    .selected_text(plugin.name)
                    .show_ui(ui, |ui| {
                        for (index, plugin) in app.plugins.entries.iter().enumerate() {
                            ui.selectable_value(&mut next_plugin, index, plugin.name);
                        }
                    });
                ui.add_enabled(
                    plugin.can_toggle(),
                    manage_button(if enabled || stopping {
                        t!("plugins.disable")
                    } else {
                        t!("plugins.enable")
                    }),
                )
                .clicked()
            })
            .inner;

        if next_plugin != selected {
            let dirty = enabled && plugin.main.has_changes();
            confirm_discard(app, dirty, move |app| {
                Self::discard_changes(app);
                app.plugin_panel.selected = next_plugin;
                app.plugin_panel.selected_tab = 0;
            });
            return false;
        }

        if toggle {
            if enabled {
                let dirty = plugin.main.has_changes();
                confirm_discard(app, dirty, move |app| {
                    app.set_plugin_enabled(key, false);
                });
            } else {
                app.set_plugin_enabled(key, true);
            }
            return false;
        }
        enabled
    }

    fn plugin_tabs(ui: &mut egui::Ui, app: &mut App) {
        let selected = app.plugin_panel.selected;
        let plugin = &mut app.plugins.entries[selected];
        let selected_tab = app.plugin_panel.selected_tab;

        let next_tab = Self::select_tab(ui, plugin.tabs, selected_tab);
        if next_tab != selected_tab {
            let dirty = plugin.main.has_changes();
            confirm_discard(app, dirty, move |app| {
                Self::discard_changes(app);
                app.plugin_panel.selected_tab = next_tab;
            });
            return;
        }

        let discard_requested =
            plugin_ui(ui, plugin, selected_tab, &app.state.settings.ui.language);
        if discard_requested {
            let dirty = plugin.main.has_changes();
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
