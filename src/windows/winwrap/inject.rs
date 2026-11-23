use std::mem::size_of;

use windows::Win32::UI::{
    Input::KeyboardAndMouse::*,
    WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    },
};

use monmouse_lib::{
    input::{InputInjection, Key, MouseAction, MouseButton, MousePos},
    keyboard::Button,
};

use super::get_last_error;
use crate::{
    errors::{Error, Result},
    keyboard::key_windows::key_to_win,
};

pub fn inject_input(input: InputInjection, extra_info: usize) -> Result<()> {
    inject_inputs(std::slice::from_ref(&input), extra_info)
}

pub fn inject_inputs(inputs: &[InputInjection], extra_info: usize) -> Result<()> {
    let mut native_inputs = Vec::new();
    for input in inputs {
        match input {
            InputInjection::Mouse { position, action } => {
                native_inputs.push(mouse_input(*position, *action, extra_info)?);
            }
            InputInjection::Keyboard { key, down } => {
                native_inputs.push(keyboard_input(*key, *down, extra_info)?);
            }
            InputInjection::Shortcut { buttons } => {
                native_inputs.extend(shortcut_inputs(buttons, extra_info)?);
            }
        }
    }
    if native_inputs.is_empty()
        || unsafe { SendInput(&native_inputs, size_of::<INPUT>() as i32) }
            == native_inputs.len() as u32
    {
        Ok(())
    } else {
        Err(get_last_error("SendInput"))
    }
}

fn shortcut_inputs(buttons: &[Button], extra_info: usize) -> Result<Vec<INPUT>> {
    let held: Vec<_> = [
        VK_LCONTROL,
        VK_RCONTROL,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
    ]
    .into_iter()
    .filter(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
    .collect();
    let mut inputs = Vec::new();
    for key in held.iter().rev() {
        inputs.push(virtual_key_input(*key, false, extra_info)?);
    }
    for button in buttons {
        inputs.push(button_input(*button, true, extra_info)?);
    }
    for button in buttons.iter().rev() {
        inputs.push(button_input(*button, false, extra_info)?);
    }
    for key in held {
        inputs.push(virtual_key_input(key, true, extra_info)?);
    }
    Ok(inputs)
}

fn button_input(button: Button, down: bool, extra_info: usize) -> Result<INPUT> {
    match button {
        Button::Keyboard(code) => {
            let key = key_to_win(code).ok_or_else(|| {
                Error::InvalidParam("shortcut".into(), format!("Unsupported key: {code:?}"))
            })?;
            virtual_key_input(key, down, extra_info)
        }
        Button::Mouse(button) => {
            mouse_input(None, MouseAction::Button { button, down }, extra_info)
        }
    }
}

fn virtual_key_input(key: VIRTUAL_KEY, down: bool, extra_info: usize) -> Result<INPUT> {
    keyboard_input(
        Key {
            virtual_key: key.0,
            scan: 0,
            extended: matches!(key.0, 0x21..=0x28 | 0x2d..=0x2e | 0x5b..=0x5d | 0xa3 | 0xa5),
        },
        down,
        extra_info,
    )
}

fn keyboard_input(key: Key, down: bool, extra_info: usize) -> Result<INPUT> {
    let use_scan = key.scan != 0 && key.virtual_key != VK_PAUSE.0;
    if key.scan > 0xff || key.virtual_key > 0xfe || (!use_scan && key.virtual_key == 0) {
        return Err(Error::InvalidParam("key".into(), format!("{key:?}")));
    }

    let mut flags = KEYBD_EVENT_FLAGS::default();
    if use_scan {
        flags |= KEYEVENTF_SCANCODE;
    }
    if key.extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if !down {
        flags |= KEYEVENTF_KEYUP;
    }
    Ok(INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(if use_scan { 0 } else { key.virtual_key }),
                wScan: if use_scan { key.scan } else { 0 },
                dwFlags: flags,
                dwExtraInfo: extra_info,
                ..Default::default()
            },
        },
    })
}

fn button_flags(button: MouseButton, down: bool) -> (MOUSE_EVENT_FLAGS, u32) {
    match (button, down) {
        (MouseButton::Left, true) => (MOUSEEVENTF_LEFTDOWN, 0),
        (MouseButton::Left, false) => (MOUSEEVENTF_LEFTUP, 0),
        (MouseButton::Right, true) => (MOUSEEVENTF_RIGHTDOWN, 0),
        (MouseButton::Right, false) => (MOUSEEVENTF_RIGHTUP, 0),
        (MouseButton::Middle, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
        (MouseButton::Middle, false) => (MOUSEEVENTF_MIDDLEUP, 0),
        (MouseButton::X1 | MouseButton::X2, down) => (
            if down {
                MOUSEEVENTF_XDOWN
            } else {
                MOUSEEVENTF_XUP
            },
            if button == MouseButton::X1 { 1 } else { 2 },
        ),
    }
}

// Aim at the center of the pixel's interval in Windows absolute coordinates.
fn absolute_coordinate(pixel: i32, origin: i32, size: i32) -> i32 {
    let offset = i64::from(pixel) - i64::from(origin);
    ((offset * 65536 + 32768) / i64::from(size)).clamp(0, 65535) as i32
}

fn mouse_input(
    position: Option<MousePos>,
    action: MouseAction,
    extra_info: usize,
) -> Result<INPUT> {
    let (flags, data) = match action {
        MouseAction::Move if position.is_none() => {
            return Err(Error::InvalidParam(
                "position".into(),
                "Mouse move requires a position".into(),
            ));
        }
        MouseAction::Move => (MOUSE_EVENT_FLAGS::default(), 0),
        MouseAction::Button { button, down } => button_flags(button, down),
        MouseAction::Wheel { delta, horizontal } => (
            if horizontal {
                MOUSEEVENTF_HWHEEL
            } else {
                MOUSEEVENTF_WHEEL
            },
            i32::from(delta) as u32,
        ),
    };
    let mut mouse = MOUSEINPUT {
        dwFlags: flags,
        mouseData: data,
        dwExtraInfo: extra_info,
        ..Default::default()
    };
    if let Some(position) = position {
        let (left, top, width, height) = unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };
        if width <= 0 || height <= 0 {
            return Err(Error::WinUnknown("GetSystemMetrics"));
        }
        mouse.dx = absolute_coordinate(position.x, left, width);
        mouse.dy = absolute_coordinate(position.y, top, height);
        mouse.dwFlags |= MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
    }
    Ok(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: mouse },
    })
}
