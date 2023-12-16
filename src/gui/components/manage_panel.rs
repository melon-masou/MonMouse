use eframe::egui;
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use monmouse::message::DeviceStatus;

use crate::{
    components::widget::{
        device_status_color, indicator_ui, manage_button, toggle_ui, CollapsingPopup,
    },
    state::DeviceUIState,
};

pub struct ManagePanel {}

impl ManagePanel {
    pub fn device_line_ui(i: usize, row: &mut egui_extras::TableRow, device: &mut DeviceUIState) {
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
            details_popup.ui(ui, "TestDevice A-100", |ui| {
                ui.label("TODO");
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
            .body(|body| {
                body.rows(20.0, devices.len(), |i, mut row| {
                    Self::device_line_ui(i, &mut row, devices.get_mut(i).unwrap());
                });
            });
    }

    pub fn ui(ui: &mut egui::Ui, devices: &mut Vec<DeviceUIState>) {
        ui.horizontal(|ui| {
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
                        Self::table_ui(ui, devices);
                    });
                });
            });
        ui.separator();

        ui.label("Help TODO");
    }
}
