use eframe::egui;

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugInfo {
    paint_times: u64,
    last_paint: u64,
    cur_paint: u64,
}

impl DebugInfo {
    pub fn on_paint_frame(&mut self, tick: u64) {
        self.paint_times += 1;
        self.last_paint = self.cur_paint;
        self.cur_paint = tick;
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        ui.label(format!("Painted: {}", self.paint_times));
        ui.label(format!("PaintCost: {}", self.cur_paint - self.last_paint));
    }
}
