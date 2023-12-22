use eframe::egui;
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use monmouse::message::{DeviceStatus, GenericDevice, Positioning};

use crate::{
    app::DeviceUIState,
    components::widget::{device_status_color, indicator_ui, manage_button, toggle_ui},
    App,
};

use super::widget::{CommonPopup, EatInputBuffer};

pub struct DevicesPanel {}

impl DevicesPanel {
    const MIN_DEVICES_ROW: usize = 15;

    fn active_str(status: &DeviceStatus) -> &str {
        match status {
            DeviceStatus::Active(positioning) => match positioning {
                Positioning::Unknown => "Active",
                Positioning::Relative => "Relative",
                Positioning::Absolute => "Absolute",
            },
            DeviceStatus::Idle => "Idle",
            DeviceStatus::Disconnected => "Disconnected",
            DeviceStatus::Unknown => "Unknown",
        }
    }

    fn device_details_text(d: &GenericDevice) -> String {
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

    fn device_line_ui(
        i: usize,
        row: &mut egui_extras::TableRow,
        device: &mut DeviceUIState,
    ) -> bool {
        let d = &device.generic;
        let mut changed = false;
        row.col(|ui| {
            indicator_ui(ui, device_status_color(ui, &device.status));
            ui.label(Self::active_str(&device.status));
        });
        row.col(|ui| {
            if toggle_ui(ui, &mut device.device_setting.switch, "switch").changed() {
                changed = true;
            }
        });
        row.col(|ui| {
            if toggle_ui(ui, &mut device.device_setting.locked_in_monitor, "locked").changed() {
                changed = true;
            }
        });
        row.col(|ui| {
            ui.label(device.generic.device_type.to_string());
            ui.add_space(10.0);
        });
        row.col(|ui| {
            let details_popup = CommonPopup::new(format!("ManagedDeviceIdx{}", i))
                .focus(true)
                .width(400.0)
                .fit_in_frame(true);

            details_popup.collapsed(ui, d.product_name.clone(), |ui, _| {
                let details_text = Self::device_details_text(&device.generic);
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
                    egui::TextEdit::multiline(&mut EatInputBuffer::from(&details_text))
                        .clip_text(false)
                        .desired_width(f32::INFINITY)
                        .frame(true),
                );
                (popup_close, ())
            });
            ui.add_space(10.0);
        });
        changed
    }

    fn table_ui(ui: &mut egui::Ui, devices: &mut Vec<DeviceUIState>) -> bool {
        let table = TableBuilder::new(ui)
            .striped(true)
            .drag_to_scroll(true)
            .auto_shrink(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::exact(100.0))
            .columns(Column::auto(), 3)
            .column(Column::remainder());
        let mut changed = false;

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Activity");
                });
                header.col(|ui| {
                    ui.strong("Switch");
                });
                header.col(|ui| {
                    ui.strong("Locked");
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
                        if Self::device_line_ui(i, &mut row, device) {
                            changed = true;
                        }
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
        changed
    }

    pub fn ui(ui: &mut egui::Ui, app: &mut App) {
        ui.horizontal(|ui| {
            if ui.add(manage_button("Scan")).clicked() {
                app.trigger_scan_devices();
            }
            if ui.add(manage_button("Save")).clicked() {
                app.save_devices_config();
            }
        });

        ui.separator();
        StripBuilder::new(ui)
            .size(Size::remainder())
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        let changed = Self::table_ui(ui, &mut app.state.managed_devices);
                        if changed {
                            app.trigger_settings_changed();
                        }
                    });
                });
            });
    }
}
