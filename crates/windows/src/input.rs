//! Keyboard and mouse through `SendInput`.

use crate::keys::{modifier_code, virtual_key};
use agentic_desktop_core::input::{Combo, MouseButton};
use agentic_desktop_core::{AppError, ErrorCode};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{SetCursorPos, SetProcessDPIAware};

const WHEEL_STEP: i32 = 120;

fn failed(message: &str) -> AppError {
    AppError::new(ErrorCode::ActionFailed, message)
}

fn key_event(code: u16, extended: bool, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(code),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse_event(flags: MOUSE_EVENT_FLAGS, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(events: &[INPUT]) -> Result<(), AppError> {
    let sent = unsafe { SendInput(events, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != events.len() {
        return Err(failed(
            "Windows blocked the input. A program running as administrator may be in front",
        ));
    }
    Ok(())
}

pub fn press(combo: &Combo) -> Result<(), AppError> {
    let key = virtual_key(combo.key).ok_or_else(|| {
        AppError::new(
            ErrorCode::ActionNotSupported,
            "This key is not available on the US layout",
        )
    })?;
    let mut held: Vec<u16> = combo.modifiers.iter().map(|m| modifier_code(*m)).collect();
    if key.needs_shift && !held.contains(&0x10) {
        held.push(0x10);
    }
    let mut events: Vec<INPUT> = held
        .iter()
        .map(|code| key_event(*code, false, false))
        .collect();
    events.push(key_event(key.code, key.extended, false));
    events.push(key_event(key.code, key.extended, true));
    events.extend(held.iter().rev().map(|code| key_event(*code, false, true)));
    send(&events)
}

pub fn mouse_move(x: f64, y: f64) -> Result<(), AppError> {
    unsafe {
        let _ = SetProcessDPIAware();
        SetCursorPos(x.round() as i32, y.round() as i32)
            .map_err(|_| failed("Cannot move the pointer"))
    }
}

pub fn mouse_click(x: f64, y: f64, button: MouseButton, count: u8) -> Result<(), AppError> {
    mouse_move(x, y)?;
    let (down, up) = match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    };
    let events: Vec<INPUT> = (0..count)
        .flat_map(|_| [mouse_event(down, 0), mouse_event(up, 0)])
        .collect();
    send(&events)
}

/// Windows counts wheel movement away from the user as positive, so down is negative.
pub fn mouse_wheel(dx: i32, dy: i32) -> Result<(), AppError> {
    let mut events = Vec::new();
    if dy != 0 {
        events.push(mouse_event(MOUSEEVENTF_WHEEL, -dy * WHEEL_STEP));
    }
    if dx != 0 {
        events.push(mouse_event(MOUSEEVENTF_HWHEEL, dx * WHEEL_STEP));
    }
    send(&events)
}
