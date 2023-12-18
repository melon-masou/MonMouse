use eframe::egui;
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use monmouse::message::{DeviceStatus, GenericDevice};

use crate::{
    components::widget::{
        device_status_color, indicator_ui, manage_button, toggle_ui, CollapsingPopup,
    },
    state::DeviceUIState,
    App,
};

pub struct DevicesPanel {}

impl DevicesPanel {
    const MIN_DEVICES_ROW: usize = 15;

    pub fn device_details_text(d: &GenericDevice) -> String {
        let mut st = String::new();
        use std::fmt::Write;
        writeln!(st, "id: {}", d.id).unwrap();
        writeln!(st, "type: {:?}", d.device_type).unwrap();
        writeln!(st, "product: {}", d.product_name).unwrap();
        writeln!(st).unwrap();
        writeln!(st, "#platform_specific_infos").unwrap();
        d.platform_specific_infos
            .iter()
            .for_each(|(tag, val)| writeln!(st, "{}: {}", tag, val).unwrap());
        st
    }

    pub fn device_line_ui(i: usize, row: &mut egui_extras::TableRow, device: &mut DeviceUIState) {
        let d = &device.generic;
        row.col(|ui| {
            indicator_ui(ui, device_status_color(ui, DeviceStatus::Active));
            ui.label("Relative");
        });
        row.col(|ui| {
            toggle_ui(ui, &mut device.switch, "switch");
        });
        row.col(|ui| {
            toggle_ui(ui, &mut device.locked, "locked");
        });
        row.col(|ui| {
            ui.label("Touch");
            ui.add_space(10.0);
        });
        row.col(|ui| {
            let details_popup = CollapsingPopup::new(format!("ManagedDeviceIdx{}", i))
                .focus(true)
                .width(400.0)
                .fit_in_frame(true);

            details_popup.ui(ui, d.product_name.clone(), |ui| {
                let mut details_text = Self::device_details_text(&device.generic);
                let mut popup_close = false;
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        popup_close = true;
                    }
                    if ui.button("Copy").clicked() {
                        ui.output_mut(|o| o.copied_text = details_text.clone());
                    }
                });
                ui.add(
                    // Have tried to use immutable TextEdit, but the frame lost even though .frame(true) is called
                    egui::TextEdit::multiline(&mut details_text)
                        .clip_text(false)
                        .desired_width(f32::INFINITY)
                        .frame(true),
                );
                popup_close
            });
            ui.add_space(10.0);
        });
    }

    pub fn table_ui(ui: &mut egui::Ui, devices: &mut Vec<DeviceUIState>) {
        let table = TableBuilder::new(ui)
            .striped(true)
            .drag_to_scroll(true)
            .auto_shrink(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::exact(60.0))
            .columns(Column::auto(), 2)
            .column(Column::exact(60.0))
            .column(Column::remainder());

        table
            .header(20.0, |mut header| {
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
                let row_height = 20.0;
                devices.iter_mut().enumerate().for_each(|(i, device)| {
                    body.row(row_height, |mut row| {
                        Self::device_line_ui(i, &mut row, device);
                    });
                });
                for _ in 0..(Self::MIN_DEVICES_ROW as isize - devices.len() as isize) {
                    body.row(20.0, |mut row| {
                        for _ in 0..5 {
                            row.col(|_| {});
                        }
                    });
                }
            });
    }

    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        ui.horizontal(|ui| {
            if manage_button(ui, "Scan").clicked() {
                app.ui_reactor.trigger_scan_devices();
            }
            if manage_button(ui, "Save").clicked() {
                // TODO
            }
        });

        ui.separator();
        StripBuilder::new(ui)
            .size(Size::remainder())
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        Self::table_ui(ui, &mut app.state.managed_devices);
                    });
                });
            });
    }
}
