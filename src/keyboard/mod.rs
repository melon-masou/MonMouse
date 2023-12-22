pub mod key_egui;
#[cfg(target_os = "windows")]
pub mod key_windows;

use keyboard_types::{Code, Modifiers};

#[inline]
pub fn modifier_or(modifier: Option<Modifiers>, m: Modifiers) -> Option<Modifiers> {
    if let Some(mut v) = modifier {
        v.insert(m);
        Some(v)
    } else {
        Some(m)
    }
}

#[cfg(target_os = "windows")]
pub const META_STR: &str = "Win+";

pub fn shortcut_to_str(m: Modifiers, code: Option<Code>) -> String {
    let mut s = String::new();
    if m.ctrl() {
        s.push_str("Ctrl+")
    }
    if m.meta() {
        s.push_str(META_STR)
    }
    if m.alt() {
        s.push_str("Alt+")
    }
    if m.shift() {
        s.push_str("Shift+")
    }
    if let Some(c) = code {
        s.push_str(key_to_str(c))
    }
    s
}

pub fn shortcut_from_str(s: &str) -> Option<(Modifiers, Code)> {
    let mut m: Option<Modifiers> = None;
    let mut key: Option<Code> = None;
    let mut last = 0;

    let mut match_one = |sub| -> bool {
        match sub {
            "Ctrl" => m = modifier_or(m, Modifiers::CONTROL),
            META_STR => m = modifier_or(m, Modifiers::META),
            "Alt" => m = modifier_or(m, Modifiers::ALT),
            "Shift" => m = modifier_or(m, Modifiers::SHIFT),
            _ => {
                if key.is_some() {
                    return false;
                }
                match str_to_key(sub) {
                    Some(k) => key = Some(k),
                    None => return false,
                }
            }
        }
        true
    };

    for (i, c) in s.chars().enumerate() {
        if c == '+' {
            if i == 0 {
                return None;
            }
            if !match_one(&s[last..i]) {
                return None;
            }
            last = i + 1;
        }
    }
    if !match_one(&s[last..]) {
        return None;
    }
    if let (Some(m), Some(key)) = (m, key) {
        Some((m, key))
    } else {
        None
    }
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
        Code::Minus => "Minus",
        Code::Equal => "Plus",
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
        "Minus" => Code::Minus,
        "Plus" => Code::Equal,
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
    fn test_shortcut_str() {
        let test_ok = |modifiers, code, str| {
            assert_eq!(shortcut_to_str(modifiers, code), str);
            if code.is_some() {
                assert_eq!(shortcut_from_str(str), Some((modifiers, code.unwrap())));
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
        // No modifier
        assert_eq!(shortcut_from_str("A"), None);
    }
}
