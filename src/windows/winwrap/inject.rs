use std::mem::size_of;

use windows::Win32::UI::{
    Input::KeyboardAndMouse::*,
    WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    },
};

use monmouse_lib::input::{InputInjection, Key, MouseAction, MouseButton, MousePos};

use super::get_last_error;
use crate::errors::{Error, Result};

pub fn inject_input(input: InputInjection, extra_info: usize) -> Result<()> {
    match input {
        InputInjection::Mouse { position, action } => inject_mouse(position, action, extra_info),
        InputInjection::Keyboard { key, down } => inject_keyboard(key, down, extra_info),
    }
}

fn send(input: INPUT) -> Result<()> {
    if unsafe { SendInput(&[input], size_of::<INPUT>() as i32) } == 1 {
        Ok(())
    } else {
        Err(get_last_error("SendInput"))
    }
}

fn inject_keyboard(key: Key, down: bool, extra_info: usize) -> Result<()> {
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
    send(INPUT {
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

fn inject_mouse(position: Option<MousePos>, action: MouseAction, extra_info: usize) -> Result<()> {
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
    send(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: mouse },
    })
}
