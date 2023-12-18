use eframe::egui;

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugInfo {
    paint_times: u64,
}

impl DebugInfo {
    pub fn on_paint(&mut self) {
        self.paint_times += 1;
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        ui.label(format!("Painted: {}", self.paint_times));
    }
}
