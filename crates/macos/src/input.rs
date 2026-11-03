//! Keyboard and mouse through CoreGraphics events.

use crate::ffi::{CFTypeRef, CGPoint, release};
use crate::keys::{FLAG_SHIFT, mac_key, modifier_flag};
use agentic_desktop_core::input::{Combo, MouseButton};
use agentic_desktop_core::{AppError, ErrorCode};
use std::time::Duration;

const HID_EVENT_TAP: u32 = 0;
const MOUSE_MOVED: u32 = 5;
const CLICK_STATE_FIELD: u32 = 1;
const SCROLL_UNIT_LINE: u32 = 1;
const PAUSE: Duration = Duration::from_millis(15);

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(source: CFTypeRef, key: u16, down: u8) -> CFTypeRef;
    fn CGEventCreateMouseEvent(
        source: CFTypeRef,
        kind: u32,
        point: CGPoint,
        button: u32,
    ) -> CFTypeRef;
    fn CGEventCreateScrollWheelEvent2(
        source: CFTypeRef,
        units: u32,
        count: u32,
        wheel1: i32,
        wheel2: i32,
        wheel3: i32,
    ) -> CFTypeRef;
    fn CGEventSetFlags(event: CFTypeRef, flags: u64);
    fn CGEventSetIntegerValueField(event: CFTypeRef, field: u32, value: i64);
    fn CGEventPost(tap: u32, event: CFTypeRef);
}

fn failed(message: &str) -> AppError {
    AppError::new(ErrorCode::ActionFailed, message)
}

/// Post an event and release it. A null event means the system refused to make it.
fn post(event: CFTypeRef) -> Result<(), AppError> {
    if event.is_null() {
        return Err(failed("The system refused to create the input event"));
    }
    unsafe {
        CGEventPost(HID_EVENT_TAP, event);
        release(event);
    }
    Ok(())
}

fn key_event(code: u16, down: bool, flags: u64) -> Result<(), AppError> {
    let event = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), code, u8::from(down)) };
    if !event.is_null() {
        unsafe { CGEventSetFlags(event, flags) };
    }
    post(event)
}

pub fn press(combo: &Combo) -> Result<(), AppError> {
    let key = mac_key(combo.key).ok_or_else(|| {
        AppError::new(
            ErrorCode::ActionNotSupported,
            "This key is not available on the US layout",
        )
    })?;
    let mut flags = combo
        .modifiers
        .iter()
        .fold(0, |sum, m| sum | modifier_flag(*m));
    if key.needs_shift {
        flags |= FLAG_SHIFT;
    }
    key_event(key.code, true, flags)?;
    key_event(key.code, false, flags)
}

fn mouse_event(kind: u32, x: f64, y: f64, button: u32, clicks: i64) -> Result<(), AppError> {
    let event =
        unsafe { CGEventCreateMouseEvent(std::ptr::null(), kind, CGPoint { x, y }, button) };
    if !event.is_null() {
        unsafe { CGEventSetIntegerValueField(event, CLICK_STATE_FIELD, clicks) };
    }
    post(event)
}

pub fn mouse_move(x: f64, y: f64) -> Result<(), AppError> {
    mouse_event(MOUSE_MOVED, x, y, 0, 0)
}

pub fn mouse_click(x: f64, y: f64, button: MouseButton, count: u8) -> Result<(), AppError> {
    mouse_move(x, y)?;
    let (down, up, number) = match button {
        MouseButton::Left => (1, 2, 0),
        MouseButton::Right => (3, 4, 1),
        MouseButton::Middle => (25, 26, 2),
    };
    for click in 1..=i64::from(count) {
        mouse_event(down, x, y, number, click)?;
        mouse_event(up, x, y, number, click)?;
        std::thread::sleep(PAUSE);
    }
    Ok(())
}

/// macOS scrolls up with a positive number, so our "down is positive" flips the sign.
pub fn mouse_wheel(dx: i32, dy: i32) -> Result<(), AppError> {
    let event = unsafe {
        CGEventCreateScrollWheelEvent2(std::ptr::null(), SCROLL_UNIT_LINE, 2, -dy, -dx, 0)
    };
    post(event)
}
