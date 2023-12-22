use keyboard_types::{Code, Modifiers};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use super::shortcut_from_str;

pub fn shortcut_str_to_win(st: &str) -> Option<(HOT_KEY_MODIFIERS, VIRTUAL_KEY)> {
    shortcut_from_str(st).and_then(|(m, code)| key_to_win(code).map(|c| (modifier_to_win(m), c)))
}

pub fn modifier_to_win(m: Modifiers) -> HOT_KEY_MODIFIERS {
    let mut r = HOT_KEY_MODIFIERS(0);
    if m.ctrl() {
        r |= MOD_CONTROL;
    }
    if m.alt() {
        r |= MOD_ALT;
    }
    if m.shift() {
        r |= MOD_SHIFT;
    }
    if m.contains(Modifiers::META) {
        r |= MOD_WIN;
    }
    r
}

pub fn key_to_win(key: Code) -> Option<VIRTUAL_KEY> {
    Some(match key {
        Code::ArrowDown => VK_DOWN,
        Code::ArrowLeft => VK_LEFT,
        Code::ArrowRight => VK_RIGHT,
        Code::ArrowUp => VK_UP,
        Code::Escape => VK_ESCAPE,
        Code::Tab => VK_TAB,
        Code::Backspace => VK_BACK,
        Code::Enter => VK_RETURN,
        Code::Space => VK_SPACE,
        Code::Insert => VK_INSERT,
        Code::Delete => VK_DELETE,
        Code::Home => VK_HOME,
        Code::End => VK_END,
        Code::PageUp => VK_PRIOR,
        Code::PageDown => VK_NEXT,
        Code::Minus => VK_OEM_MINUS,
        Code::Equal => VK_OEM_PLUS,
        Code::Digit0 => VK_0,
        Code::Digit1 => VK_1,
        Code::Digit2 => VK_2,
        Code::Digit3 => VK_3,
        Code::Digit4 => VK_4,
        Code::Digit5 => VK_5,
        Code::Digit6 => VK_6,
        Code::Digit7 => VK_7,
        Code::Digit8 => VK_8,
        Code::Digit9 => VK_9,
        Code::KeyA => VK_A,
        Code::KeyB => VK_B,
        Code::KeyC => VK_C,
        Code::KeyD => VK_D,
        Code::KeyE => VK_E,
        Code::KeyF => VK_F,
        Code::KeyG => VK_G,
        Code::KeyH => VK_H,
        Code::KeyI => VK_I,
        Code::KeyJ => VK_J,
        Code::KeyK => VK_K,
        Code::KeyL => VK_L,
        Code::KeyM => VK_M,
        Code::KeyN => VK_N,
        Code::KeyO => VK_O,
        Code::KeyP => VK_P,
        Code::KeyQ => VK_Q,
        Code::KeyR => VK_R,
        Code::KeyS => VK_S,
        Code::KeyT => VK_T,
        Code::KeyU => VK_U,
        Code::KeyV => VK_V,
        Code::KeyW => VK_W,
        Code::KeyX => VK_X,
        Code::KeyY => VK_Y,
        Code::KeyZ => VK_Z,
        Code::F1 => VK_F1,
        Code::F2 => VK_F2,
        Code::F3 => VK_F3,
        Code::F4 => VK_F4,
        Code::F5 => VK_F5,
        Code::F6 => VK_F6,
        Code::F7 => VK_F7,
        Code::F8 => VK_F8,
        Code::F9 => VK_F9,
        Code::F10 => VK_F10,
        Code::F11 => VK_F11,
        Code::F12 => VK_F12,
        Code::F13 => VK_F13,
        Code::F14 => VK_F14,
        Code::F15 => VK_F15,
        Code::F16 => VK_F16,
        Code::F17 => VK_F17,
        Code::F18 => VK_F18,
        Code::F19 => VK_F19,
        Code::F20 => VK_F20,
        _ => return None,
    })
}
