use eframe::{
    egui::{self, Widget},
    epaint::Color32,
};
use monmouse::{
    keyboard::{
        build_modifiers,
        key_egui::{egui_to_key, egui_to_modifier},
        shortcut_to_str, META_STR,
    },
    message::DeviceStatus,
};

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

pub fn manage_button(text: &str) -> egui::Button {
    let text = egui::RichText::new(text).strong();
    egui::Button::new(text).min_size(egui::vec2(70.0, 25.0))
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

//Codes derived from:
// https://github.com/emilk/egui/blob/0.24.1/crates/egui_demo_lib/src/demo/toggle_switch.rs
//Under MIT license:
// Copyright (c) 2018-2021 Emil Ernerfeldt <emil.ernerfeldt@gmail.com>

// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:

// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
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
pub struct CommonPopupState {
    will_close: bool,
    open: bool,
}

pub struct CommonPopupResponse<T> {
    pub header_response: egui::Response,
    pub popup_response: Option<(bool, T)>,
}

pub struct CommonPopup {
    id_source: egui::Id,
    width: f32,
    focus: bool,
    fixed_pos: Option<egui::Pos2>,
    fit_in_frame: bool,
}

impl CommonPopup {
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

    pub fn collapsed<T>(
        self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        popup_ui: impl FnOnce(&mut egui::Ui, bool /* just_open */) -> (bool, T),
    ) -> CommonPopupResponse<T> {
        let id_source = self.id_source;
        self.ui(
            ui,
            |ui, open_state| {
                let collapsing = egui::CollapsingHeader::new(text)
                    .id_source(id_source)
                    .open(open_state);
                let collapsing_response = collapsing.show(ui, |_| {
                    // Add nothing into body, create popup after collapsing is fully opened
                });
                (
                    Some(collapsing_response.fully_open()),
                    collapsing_response.header_response,
                )
            },
            popup_ui,
        )
    }

    pub fn ui<T>(
        self,
        ui: &mut egui::Ui,
        header_ui: impl FnOnce(&mut egui::Ui, Option<bool>) -> (Option<bool>, egui::Response),
        popup_ui: impl FnOnce(&mut egui::Ui, bool) -> (bool, T),
    ) -> CommonPopupResponse<T> {
        let id = ui.make_persistent_id(self.id_source);
        let mut state = ui
            .memory_mut(|mem| mem.data.get_persisted::<CommonPopupState>(id))
            .unwrap_or_default();

        let open_state = if state.will_close {
            state.will_close = false;
            ui.memory_mut(|mem| mem.data.insert_persisted(id, state.clone()));
            Some(false)
        } else {
            None
        };

        let mut just_open = false;
        let (open_state, response) = header_ui(ui, open_state);
        if let Some(o) = open_state {
            if state.open != o {
                state.open = o;
                just_open = o;
                ui.memory_mut(|mem| mem.data.insert_persisted(id, state.clone()));
            }
        }

        let mut popup_response: Option<(bool, T)> = None;
        if state.open {
            let pos = self.popup_pos(ui, &response.rect);

            let mut area = egui::Area::new(id)
                .order(egui::Order::Foreground)
                .fixed_pos(pos);
            if self.fit_in_frame {
                area = area.constrain_to(ui.ctx().screen_rect());
            }
            let egui::InnerResponse {
                inner: mut popup_return_close,
                response: area_response,
            } = area.show(ui.ctx(), |ui| {
                let frame = egui::Frame::popup(ui.style());
                frame.show(ui, |ui| {
                    ui.set_min_width(self.width);
                    ui.set_max_width(self.width);
                    popup_ui(ui, just_open)
                })
            });

            let will_close = popup_return_close.inner.0
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                || (!just_open && self.focus && area_response.clicked_elsewhere());
            if will_close {
                state.will_close = true;
                ui.memory_mut(|mem| mem.data.insert_persisted(id, state));
            }
            popup_return_close.inner.0 = will_close;
            popup_response = Some(popup_return_close.inner);
        }

        CommonPopupResponse {
            header_response: response,
            popup_response,
        }
    }
}

pub struct ShortcutInputResponse {
    pub focus: bool,
    pub changed: bool,
}

pub fn shortcut_input_ui(
    ui: &mut egui::Ui,
    buf: &mut String,
    show_modifier: bool,
    textinput_style: impl FnOnce(egui::TextEdit) -> egui::TextEdit,
) -> ShortcutInputResponse {
    let mut b = EatInputBuffer::from(buf);
    let textinput = textinput_style(egui::TextEdit::singleline(&mut b).desired_width(140.0));

    let inner = textinput.ui(ui);
    let focus = inner.has_focus();

    if focus {
        let (modifiers, key) =
            ui.input(|input| (input.modifiers, input.keys_down.iter().next().cloned()));
        let new_shortcut = shortcut_to_str(
            if show_modifier {
                egui_to_modifier(modifiers)
            } else {
                None
            },
            key.map(egui_to_key),
        );
        *buf = new_shortcut;
        // Had key, stop input
        if key.is_some() {
            ui.memory_mut(|mem| mem.stop_text_input());
        }
        return ShortcutInputResponse {
            focus,
            changed: key.is_some(),
        };
    }

    ShortcutInputResponse {
        focus,
        changed: false,
    }
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct ShortcutChooseState {
    key_input: String,
    ctrl_checked: bool,
    meta_checked: bool,
    shift_checked: bool,
    alt_checked: bool,
}

pub struct ShortcutChoosePopup {
    id_source: egui::Id,
}

impl ShortcutChoosePopup {
    pub fn new(id_source: impl std::hash::Hash) -> Self {
        Self {
            id_source: egui::Id::new(id_source),
        }
    }

    pub fn button_ui(
        ui: &mut egui::Ui,
        open_state: Option<bool>,
        text: &str,
    ) -> (Option<bool>, egui::Response) {
        let resp = ui.add(egui::Button::new(text).min_size(egui::vec2(140.0, 10.0)));
        (
            if resp.clicked() {
                Some(true)
            } else {
                open_state
            },
            resp,
        )
    }

    pub fn popup_ui(&mut self, ui: &mut egui::Ui, just_open: bool) -> (bool, ShortcutChooseState) {
        let id = ui.make_persistent_id(self.id_source);
        let mut state = if just_open {
            ui.memory_mut(|mem| mem.data.remove::<ShortcutChooseState>(id));
            ShortcutChooseState::default()
        } else {
            ui.memory_mut(|mem| mem.data.get_persisted::<ShortcutChooseState>(id))
                .unwrap_or_default()
        };

        let mut changed = false;
        changed |= ui.checkbox(&mut state.ctrl_checked, "Ctrl").clicked();
        changed |= ui.checkbox(&mut state.meta_checked, META_STR).clicked();
        changed |= ui.checkbox(&mut state.shift_checked, "Shift").clicked();
        changed |= ui.checkbox(&mut state.alt_checked, "Alt").clicked();

        changed |= shortcut_input_ui(ui, &mut state.key_input, false, |textinput| {
            textinput.desired_width(50.0)
        })
        .changed;

        if changed {
            ui.memory_mut(|mem| mem.data.insert_persisted(id, state.clone()));
        }
        (false, state)
    }

    pub fn short_cut_from_state(&mut self, state: ShortcutChooseState) -> String {
        if state.key_input.is_empty() {
            return "".to_owned();
        }

        let modifiers = match build_modifiers(
            state.ctrl_checked,
            state.alt_checked,
            state.shift_checked,
            state.meta_checked,
        ) {
            Some(v) => v,
            None => return "".to_owned(),
        };
        let mut s = shortcut_to_str(Some(modifiers), None);
        s.push_str(state.key_input.as_str());
        s
    }

    pub fn ui(mut self, ui: &mut egui::Ui, buf: &mut String) -> ShortcutInputResponse {
        let resp = CommonPopup::new(self.id_source).ui(
            ui,
            |ui, open_state| Self::button_ui(ui, open_state, buf.as_str()),
            |ui, just_open| self.popup_ui(ui, just_open),
        );
        let mut r = ShortcutInputResponse {
            focus: false,
            changed: false,
        };
        let (will_close, state) = match resp.popup_response {
            Some(v) => (v.0, v.1),
            None => return r,
        };
        if will_close {
            *buf = self.short_cut_from_state(state);
        }
        r.changed |= will_close;
        r
    }
}

// A workaround to make egui editable TextEdit not "edited" by itself
pub struct EatInputBuffer<'a> {
    buf: &'a str,
}

impl<'a> EatInputBuffer<'a> {
    pub fn from(buf: &'a str) -> Self {
        Self { buf }
    }
}

impl<'a> egui::TextBuffer for EatInputBuffer<'a> {
    fn is_mutable(&self) -> bool {
        true
    }
    fn as_str(&self) -> &str {
        self.buf
    }
    fn insert_text(&mut self, text: &str, _char_index: usize) -> usize {
        text.len()
    }
    fn delete_char_range(&mut self, _char_range: std::ops::Range<usize>) {}
}
