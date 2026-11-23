use crate::input::MouseButton;
pub use keyboard_types::{Code, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Keyboard(Code),
    Mouse(MouseButton),
}

pub fn parse_chord(text: &str, allow_mouse: bool) -> Result<Vec<Button>, String> {
    let mut keys = Vec::new();
    for token in text.split('+').map(str::trim) {
        let key = match token {
            "Ctrl" => Button::Keyboard(Code::ControlLeft),
            "Shift" => Button::Keyboard(Code::ShiftLeft),
            "Alt" => Button::Keyboard(Code::AltLeft),
            "Win" => Button::Keyboard(Code::MetaLeft),
            "CapsLock" => Button::Keyboard(Code::CapsLock),
            "MouseLeft" => Button::Mouse(MouseButton::Left),
            "MouseRight" => Button::Mouse(MouseButton::Right),
            "MouseMiddle" => Button::Mouse(MouseButton::Middle),
            "MouseX1" => Button::Mouse(MouseButton::X1),
            "MouseX2" => Button::Mouse(MouseButton::X2),
            _ => {
                Button::Keyboard(str_to_key(token).ok_or_else(|| format!("Unknown key: {token}"))?)
            }
        };
        if !allow_mouse && matches!(key, Button::Mouse(_)) {
            return Err("Mouse buttons are not allowed in this shortcut".to_owned());
        }
        if keys.contains(&key) {
            return Err(format!("Duplicate key: {token}"));
        }
        keys.push(key);
    }
    keys.sort_by_key(|key| {
        !matches!(
            key,
            Button::Keyboard(Code::ControlLeft | Code::ShiftLeft | Code::AltLeft | Code::MetaLeft)
        )
    });
    Ok(keys)
}

#[inline]
pub fn modifier_or(modifier: Option<Modifiers>, m: Modifiers) -> Option<Modifiers> {
    if let Some(mut v) = modifier {
        v.insert(m);
        Some(v)
    } else {
        Some(m)
    }
}

pub fn build_modifiers(ctrl: bool, alt: bool, shift: bool, meta: bool) -> Option<Modifiers> {
    let mut m: Option<Modifiers> = None;
    if ctrl {
        m = modifier_or(m, Modifiers::CONTROL);
    }
    if alt {
        m = modifier_or(m, Modifiers::ALT);
    }
    if shift {
        m = modifier_or(m, Modifiers::SHIFT);
    }
    if meta {
        m = modifier_or(m, Modifiers::META);
    }
    m
}

pub const META_STR: &str = "Win";

pub fn shortcut_to_str(modifiers: Option<Modifiers>, code: Option<Code>) -> String {
    let mut s = String::new();
    if let Some(m) = modifiers {
        if m.ctrl() {
            s.push_str("Ctrl+")
        }
        if m.meta() {
            s.push_str(META_STR);
            s.push('+');
        }
        if m.alt() {
            s.push_str("Alt+")
        }
        if m.shift() {
            s.push_str("Shift+")
        }
    }
    if let Some(c) = code {
        s.push_str(key_to_str(c))
    }
    s
}

pub fn shortcut_from_str(s: &str) -> Option<(Modifiers, Code)> {
    let mut modifiers = Modifiers::empty();
    let mut key = None;
    for button in parse_chord(s, false).ok()? {
        match button {
            Button::Keyboard(Code::ControlLeft) => modifiers |= Modifiers::CONTROL,
            Button::Keyboard(Code::ShiftLeft) => modifiers |= Modifiers::SHIFT,
            Button::Keyboard(Code::AltLeft) => modifiers |= Modifiers::ALT,
            Button::Keyboard(Code::MetaLeft) => modifiers |= Modifiers::META,
            Button::Keyboard(code) if key.is_none() => key = Some(code),
            _ => return None,
        }
    }
    key.map(|key| (modifiers, key))
}

pub fn key_is_allowed(key: Code) -> bool {
    key_to_str(key) != "Unknown"
}

pub fn key_to_str(key: Code) -> &'static str {
    match key {
        Code::ArrowDown => "Down",
        Code::ArrowLeft => "Left",
        Code::ArrowRight => "Right",
        Code::ArrowUp => "Up",
        Code::Escape => "Escape",
        Code::Tab => "Tab",
        Code::Backspace => "Backspace",
        Code::Enter => "Enter",
        Code::Space => "Space",
        Code::Insert => "Insert",
        Code::Delete => "Delete",
        Code::Home => "Home",
        Code::End => "End",
        Code::PageUp => "PageUp",
        Code::PageDown => "PageDown",
        Code::Minus => "-",
        Code::Equal => "=",
        Code::Comma => ",",
        Code::Backslash => "\\",
        Code::Slash => "/",
        Code::BracketLeft => "[",
        Code::BracketRight => "]",
        Code::Backquote => "`",
        Code::Period => ".",
        Code::Semicolon => ";",
        Code::Quote => "'",
        Code::Digit0 => "0",
        Code::Digit1 => "1",
        Code::Digit2 => "2",
        Code::Digit3 => "3",
        Code::Digit4 => "4",
        Code::Digit5 => "5",
        Code::Digit6 => "6",
        Code::Digit7 => "7",
        Code::Digit8 => "8",
        Code::Digit9 => "9",
        Code::KeyA => "A",
        Code::KeyB => "B",
        Code::KeyC => "C",
        Code::KeyD => "D",
        Code::KeyE => "E",
        Code::KeyF => "F",
        Code::KeyG => "G",
        Code::KeyH => "H",
        Code::KeyI => "I",
        Code::KeyJ => "J",
        Code::KeyK => "K",
        Code::KeyL => "L",
        Code::KeyM => "M",
        Code::KeyN => "N",
        Code::KeyO => "O",
        Code::KeyP => "P",
        Code::KeyQ => "Q",
        Code::KeyR => "R",
        Code::KeyS => "S",
        Code::KeyT => "T",
        Code::KeyU => "U",
        Code::KeyV => "V",
        Code::KeyW => "W",
        Code::KeyX => "X",
        Code::KeyY => "Y",
        Code::KeyZ => "Z",
        Code::F1 => "F1",
        Code::F2 => "F2",
        Code::F3 => "F3",
        Code::F4 => "F4",
        Code::F5 => "F5",
        Code::F6 => "F6",
        Code::F7 => "F7",
        Code::F8 => "F8",
        Code::F9 => "F9",
        Code::F10 => "F10",
        Code::F11 => "F11",
        Code::F12 => "F12",
        Code::F13 => "F13",
        Code::F14 => "F14",
        Code::F15 => "F15",
        Code::F16 => "F16",
        Code::F17 => "F17",
        Code::F18 => "F18",
        Code::F19 => "F19",
        Code::F20 => "F20",
        _ => "Unknown",
    }
}

pub fn str_to_key(str: &str) -> Option<Code> {
    Some(match str {
        "Down" => Code::ArrowDown,
        "Left" => Code::ArrowLeft,
        "Right" => Code::ArrowRight,
        "Up" => Code::ArrowUp,
        "Escape" => Code::Escape,
        "Tab" => Code::Tab,
        "Backspace" => Code::Backspace,
        "Enter" => Code::Enter,
        "Space" => Code::Space,
        "Insert" => Code::Insert,
        "Delete" => Code::Delete,
        "Home" => Code::Home,
        "End" => Code::End,
        "PageUp" => Code::PageUp,
        "PageDown" => Code::PageDown,
        "-" => Code::Minus,
        "=" => Code::Equal,
        "," => Code::Comma,
        "\\" => Code::Backslash,
        "/" => Code::Slash,
        "[" => Code::BracketLeft,
        "]" => Code::BracketRight,
        "`" => Code::Backquote,
        "." => Code::Period,
        ";" => Code::Semicolon,
        "'" => Code::Quote,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "A" => Code::KeyA,
        "B" => Code::KeyB,
        "C" => Code::KeyC,
        "D" => Code::KeyD,
        "E" => Code::KeyE,
        "F" => Code::KeyF,
        "G" => Code::KeyG,
        "H" => Code::KeyH,
        "I" => Code::KeyI,
        "J" => Code::KeyJ,
        "K" => Code::KeyK,
        "L" => Code::KeyL,
        "M" => Code::KeyM,
        "N" => Code::KeyN,
        "O" => Code::KeyO,
        "P" => Code::KeyP,
        "Q" => Code::KeyQ,
        "R" => Code::KeyR,
        "S" => Code::KeyS,
        "T" => Code::KeyT,
        "U" => Code::KeyU,
        "V" => Code::KeyV,
        "W" => Code::KeyW,
        "X" => Code::KeyX,
        "Y" => Code::KeyY,
        "Z" => Code::KeyZ,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        "F13" => Code::F13,
        "F14" => Code::F14,
        "F15" => Code::F15,
        "F16" => Code::F16,
        "F17" => Code::F17,
        "F18" => Code::F18,
        "F19" => Code::F19,
        "F20" => Code::F20,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_parser_supports_single_keys_combinations_and_mouse_buttons() {
        assert_eq!(
            parse_chord("J", false).unwrap(),
            vec![Button::Keyboard(Code::KeyJ)]
        );
        assert_eq!(
            parse_chord("J+Ctrl+MouseRight", true).unwrap(),
            vec![
                Button::Keyboard(Code::ControlLeft),
                Button::Keyboard(Code::KeyJ),
                Button::Mouse(MouseButton::Right),
            ]
        );
        assert_eq!(
            parse_chord("Ctrl+MouseLeft", true).unwrap(),
            vec![
                Button::Keyboard(Code::ControlLeft),
                Button::Mouse(MouseButton::Left),
            ]
        );
        assert_eq!(
            parse_chord("J+K", false).unwrap(),
            vec![Button::Keyboard(Code::KeyJ), Button::Keyboard(Code::KeyK),]
        );
    }

    #[test]
    fn malformed_or_duplicate_chord_keys_are_rejected() {
        assert!(parse_chord("MouseLeft", false).is_err());
        assert!(parse_chord("Ctrl+MouseRight", false).is_err());
        for text in [
            "",
            "+J",
            "Ctrl++J",
            "Ctrl+J+Ctrl",
            "J+J",
            "MouseLeft+MouseLeft",
            "UnknownKey",
        ] {
            assert!(
                parse_chord(text, true).is_err(),
                "Accepted invalid chord: {text}"
            );
        }
    }

    #[test]
    fn test_shortcut_str() {
        let test_ok = |modifiers, code, str| {
            assert_eq!(shortcut_to_str(Some(modifiers), code), str);
            if let Some(code) = code {
                assert_eq!(shortcut_from_str(str), Some((modifiers, code)));
            } else {
                assert_eq!(shortcut_from_str(str), None);
            }
        };

        test_ok(
            Modifiers::CONTROL | Modifiers::ALT,
            Some(Code::F9),
            "Ctrl+Alt+F9",
        );
        test_ok(
            Modifiers::SHIFT | Modifiers::ALT,
            Some(Code::Home),
            "Alt+Shift+Home",
        );
        test_ok(Modifiers::SHIFT | Modifiers::ALT, None, "Alt+Shift+");

        // different order
        assert_eq!(
            shortcut_from_str("Shift+Alt+Ctrl+3"),
            Some((
                Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT,
                Code::Digit3
            ))
        );
        // start with plus
        assert_eq!(shortcut_from_str("+Shift+3"), None);
        // Invalid
        assert_eq!(shortcut_from_str("Ctrl+Shift+GI"), None);
        // End with plus
        assert_eq!(shortcut_from_str("Ctrl+Shift+4+"), None);
        // No key
        assert_eq!(shortcut_from_str("Ctrl+Shift"), None);
        // Multiple key
        assert_eq!(shortcut_from_str("Ctrl+Shift+A+D"), None);
        // Single-key hotkeys do not require a modifier.
        assert_eq!(
            shortcut_from_str("A"),
            Some((Modifiers::empty(), Code::KeyA))
        );
    }
}
