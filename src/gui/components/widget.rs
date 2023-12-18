use eframe::{egui, epaint::Color32};
use monmouse::message::DeviceStatus;

#[inline]
fn theme_red(dark: bool) -> Color32 {
    if dark {
        Color32::DARK_RED
    } else {
        Color32::LIGHT_RED
    }
}

#[inline]
fn theme_green(dark: bool) -> Color32 {
    if dark {
        Color32::DARK_GREEN
    } else {
        Color32::LIGHT_GREEN
    }
}

pub fn error_color(ui: &egui::Ui, ok: bool) -> Color32 {
    let dark = ui.style().visuals.dark_mode;
    if ok {
        theme_green(dark)
    } else {
        theme_red(dark)
    }
}

pub fn device_status_color(ui: &egui::Ui, s: &DeviceStatus) -> Color32 {
    let dark = ui.style().visuals.dark_mode;
    match s {
        DeviceStatus::Active { .. } => theme_green(dark),
        DeviceStatus::Idle => ui.style().visuals.widgets.inactive.bg_fill,
        DeviceStatus::Disconnected => theme_red(dark),
        DeviceStatus::Unknown => ui.style().visuals.widgets.noninteractive.bg_fill,
    }
}

pub fn manage_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let text = egui::RichText::new(text).strong();
    let button = egui::Button::new(text).min_size(egui::vec2(70.0, 25.0));
    ui.add(button)
}

pub fn indicator_ui(ui: &mut egui::Ui, color: impl Into<Color32>) -> egui::Response {
    let size = ui.spacing().interact_size.y * (egui::vec2(0.5, 1.0));
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::focusable_noninteractive());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().noninteractive();
        ui.painter().circle(
            rect.center(),
            0.5 * 0.5 * rect.height(),
            color,
            egui::Stroke::new(0.5, visuals.fg_stroke.color),
        );
    }

    response
}

// Codes derived from, under MIT license:
//   https://github.com/emilk/egui/blob/0.24.1/crates/egui_demo_lib/src/demo/toggle_switch.rs
pub fn toggle_ui(ui: &mut egui::Ui, on: &mut bool, label: impl ToString) -> egui::Response {
    let size = ui.spacing().interact_size.y * (egui::vec2(2.0, 1.0));
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, label.to_string())
    });

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        ui.painter()
            .rect(rect, radius, visuals.bg_fill, visuals.bg_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }

    response
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct CollapsingPopupState {
    will_close: bool,
}

pub struct CollapsingPopup {
    id_source: egui::Id,
    width: f32,
    focus: bool,
    fixed_pos: Option<egui::Pos2>,
    fit_in_frame: bool,
}

impl CollapsingPopup {
    pub fn new(id_source: impl std::hash::Hash) -> Self {
        Self {
            id_source: egui::Id::new(id_source),
            width: 300.0,
            focus: true,
            fixed_pos: None,
            fit_in_frame: true,
        }
    }

    // If set to true, The popup will be closed when clicking outside the popup area.
    #[allow(dead_code)]
    pub fn focus(mut self, value: bool) -> Self {
        self.focus = value;
        self
    }
    #[allow(dead_code)]
    pub fn fit_in_frame(mut self, value: bool) -> Self {
        self.fit_in_frame = value;
        self
    }
    // Set fixed position of the popup window
    #[allow(dead_code)]
    pub fn fixed_pos(mut self, fixed_pos: impl Into<egui::Pos2>) -> Self {
        self.fixed_pos = Some(fixed_pos.into());
        self
    }
    // Set width of the popup window
    #[allow(dead_code)]
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    fn popup_pos(&self, ui: &mut egui::Ui, header_rect: &egui::Rect) -> egui::Pos2 {
        let mut pos = if let Some(p) = self.fixed_pos {
            p
        } else {
            header_rect.left_bottom()
        };

        let width_with_padding = self.width
            + ui.style().spacing.item_spacing.x
            + ui.style().spacing.window_margin.left
            + ui.style().spacing.window_margin.right;
        if self.fit_in_frame {
            pos.x = pos
                .x
                .min(ui.clip_rect().right() - width_with_padding)
                .max(ui.clip_rect().left() + ui.style().spacing.window_margin.left);
        }
        pos
    }

    pub fn ui(
        self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        popup_ui: impl FnOnce(&mut egui::Ui) -> bool,
    ) -> egui::Response {
        let id = ui.make_persistent_id(self.id_source);
        let mut state = ui
            .memory_mut(|mem| mem.data.get_persisted::<CollapsingPopupState>(id))
            .unwrap_or_default();

        let open_state = if state.will_close {
            state.will_close = false;
            ui.memory_mut(|mem| mem.data.insert_persisted(id, state.clone()));
            Some(false)
        } else {
            None
        };

        let collapsing = egui::CollapsingHeader::new(text)
            .id_source(self.id_source)
            .open(open_state);
        let collapsing_response = collapsing.show(ui, |_| {
            // Add nothing into body, create popup after collapsing is fully opened
        });
        let fully_open = collapsing_response.fully_open();

        if fully_open {
            let pos = self.popup_pos(ui, &collapsing_response.header_response.rect);

            let mut area = egui::Area::new(id)
                .order(egui::Order::Foreground)
                .fixed_pos(pos);
            if self.fit_in_frame {
                area = area.constrain_to(ui.ctx().screen_rect());
            }
            let egui::InnerResponse {
                inner: popup_return_close,
                response: area_response,
            } = area.show(ui.ctx(), |ui| {
                let frame = egui::Frame::popup(ui.style());
                frame.show(ui, |ui| {
                    ui.set_min_width(self.width);
                    ui.set_max_width(self.width);
                    popup_ui(ui)
                })
            });

            let will_close = popup_return_close.inner
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                || (self.focus && area_response.clicked_elsewhere());
            if will_close {
                state.will_close = true;
                ui.memory_mut(|mem| mem.data.insert_persisted(id, state.clone()));
            }
        }

        collapsing_response.header_response
    }
}
