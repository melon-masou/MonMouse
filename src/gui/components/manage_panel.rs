use eframe::egui;
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use monmouse::message::DeviceStatus;

use crate::{
    components::widget::{
        device_status_color, indicator_ui, manage_button, toggle_ui, CollapsingPopup,
    },
    state::DeviceUIState,
    App,
};

pub struct ManagePanel {}

impl ManagePanel {
    const MIN_ROW: usize = 15;
    pub fn device_line_ui(i: usize, row: &mut egui_extras::TableRow, device: &mut DeviceUIState) {
        let d = &device.generic;
        row.col(|ui| {
            ui.checkbox(&mut device.checked, "");
        });
        row.col(|ui| {
            indicator_ui(ui, device_status_color(ui, DeviceStatus::Active));
            ui.label("Relative");
        });
        row.col(|ui| {
            toggle_ui(ui, &mut device.locked, "locked");
        });
        row.col(|ui| {
            toggle_ui(ui, &mut device.switch, "switch");
        });
        row.col(|ui| {
            ui.label("Touch");
            ui.add_space(10.0);
        });
        row.col(|ui| {
            let details_popup = CollapsingPopup::new(format!("ManagedDeviceIdx{}", i)).focus(true);
            details_popup.ui(ui, d.product_name.clone(), |ui| {
                ui.label("id: ");
                ui.label(d.id.clone());
            });
            ui.add_space(10.0);
        });
    }

    pub fn table_ui(ui: &mut egui::Ui, devices: &mut Vec<DeviceUIState>) {
        let table = TableBuilder::new(ui)
            .striped(true)
            // .resizable(true)
            .min_scrolled_height(100.0)
            .max_scroll_height(300.0)
            .drag_to_scroll(true)
            .auto_shrink(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto())
            .column(Column::exact(60.0))
            .columns(Column::auto(), 2)
            .column(Column::exact(60.0))
            .column(Column::remainder());

        table
            .header(20.0, |mut header| {
                header.col(|_| {});
                header.col(|ui| {
                    ui.strong("Active");
                });
                header.col(|ui| {
                    ui.strong("Locked");
                });
                header.col(|ui| {
                    ui.strong("Switch");
                });
                header.col(|ui| {
                    ui.strong("Type");
                });
                header.col(|ui| {
                    ui.strong("Product");
                });
            })
            .body(|mut body| {
                for i in 0..devices.len().max(Self::MIN_ROW) {
                    body.row(20.0, |mut row| {
                        if i < devices.len() {
                            Self::device_line_ui(i, &mut row, devices.get_mut(i).unwrap());
                        } else {
                            for _ in 0..6 {
                                row.col(|_| {});
                            }
                        }
                    });
                }
            });
    }

    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        ui.horizontal(|ui| {
            if manage_button(ui, "Refresh").clicked() {
                app.ui_reactor.trigger_inspect_devices();
            }
            if manage_button(ui, "Unmanage").clicked() {
                // TODO
            }
            if manage_button(ui, "Save").clicked() {
                // TODO
            }
        });

        ui.separator();
        StripBuilder::new(ui)
            .size(Size::exact(320.0))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        Self::table_ui(ui, &mut app.state.managed_devices);
                    });
                });
            });
        ui.separator();

        ui.label("Help TODO");
    }
}
