use eframe::egui;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AboutPanel {}

impl AboutPanel {
    pub fn ui(ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("MonMouse").strong().size(20.0));
        });
        egui::Grid::new("AboutGrids")
            .num_columns(2)
            .striped(false)
            .spacing([15.0, 3.0])
            .show(ui, |ui| {
                ui.label("Version");
                ui.label(format!("v{}", VERSION));
                ui.end_row();

                ui.label("License");
                ui.add(egui::Hyperlink::from_label_and_url(
                    "MIT",
                    "https://opensource.org/licenses/MIT",
                ));
                ui.end_row();

                ui.label("Source");
                ui.add(egui::Hyperlink::from_label_and_url(
                    "Repo",
                    "https://github.com/melon-masou/MonMouse",
                ));
                ui.end_row();
            });
    }
}
