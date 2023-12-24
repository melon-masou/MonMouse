use eframe::egui;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AboutPanel {}

impl AboutPanel {
    pub fn ui(ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("MonMouse").strong().size(20.0));
            ui.label(egui::RichText::new(format!("v{}", VERSION)).size(20.0));
        });
        ui.horizontal(|ui| {
            ui.label("License");
            ui.add(egui::Hyperlink::from_label_and_url(
                "MIT",
                "https://opensource.org/licenses/MIT",
            ));
        });
        ui.horizontal(|ui| {
            ui.label("Source");
            ui.add(egui::Hyperlink::from_label_and_url(
                "Repo",
                "https://github.com/melon-masou/MonMouse",
            ));
        });
    }
}
