use crate::keyboard::key_windows::key_to_win;
use monmouse_lib::{
    input::{InputInjection, Key, MouseAction},
    keyboard::Button,
};

pub fn shortcut_inputs(shortcut: &[Button], down: bool) -> Result<Vec<InputInjection>, String> {
    let mut inputs = shortcut
        .iter()
        .map(|&button| button_input(button, down))
        .collect::<Result<Vec<_>, _>>()?;
    if !down {
        inputs.reverse();
    }
    Ok(inputs)
}

pub fn shortcut_click_inputs(shortcut: &[Button]) -> Result<Vec<InputInjection>, String> {
    let mut inputs = shortcut_inputs(shortcut, true)?;
    inputs.extend(shortcut_inputs(shortcut, false)?);
    Ok(inputs)
}

fn button_input(button: Button, down: bool) -> Result<InputInjection, String> {
    match button {
        Button::Keyboard(code) => {
            let virtual_key = key_to_win(code)
                .ok_or_else(|| format!("Unsupported key: {code:?}"))?
                .0;
            Ok(InputInjection::Keyboard {
                key: Key {
                    scan: 0,
                    virtual_key,
                    extended: matches!(virtual_key, 0x21..=0x28 | 0x2d..=0x2e | 0x5b..=0x5d | 0xa3 | 0xa5),
                },
                down,
            })
        }
        Button::Mouse(button) => Ok(InputInjection::Mouse {
            position: None,
            action: MouseAction::Button { button, down },
        }),
    }
}
