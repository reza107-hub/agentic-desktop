//! Keyboard and mouse through the X11 test extension (XTEST). X11 sessions only.

use crate::keys::{
    KeyRoute, char_keysym, find_key, input_block_reason, key_route, keysym_for, modifier_keysym,
};
use agentic_desktop_core::input::{Combo, MouseButton};
use agentic_desktop_core::{AppError, ErrorCode};
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, ConnectionExt as _, KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT, MOTION_NOTIFY_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

const SHIFT_KEYSYM: u32 = 0xffe1;
const PAUSE: Duration = Duration::from_millis(15);

fn blocked() -> Option<AppError> {
    let session = std::env::var("XDG_SESSION_TYPE").ok();
    let reason = input_block_reason(
        session.as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    )?;
    Some(AppError::new(ErrorCode::PlatformNotSupported, reason))
}

fn x_error(what: &str, error: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::ActionFailed, format!("{what}: {error}"))
}

struct Display {
    connection: RustConnection,
    root: u32,
    first_keycode: u8,
    per_key: usize,
    symbols: Vec<u32>,
}

impl Display {
    fn open() -> Result<Display, AppError> {
        if let Some(error) = blocked() {
            return Err(error);
        }
        let (connection, screen_number) =
            x11rb::connect(None).map_err(|e| x_error("Cannot open the X11 display", e))?;
        let root = connection.setup().roots[screen_number].root;
        let first = connection.setup().min_keycode;
        let last = connection.setup().max_keycode;
        let map = connection
            .get_keyboard_mapping(first, last - first + 1)
            .map_err(|e| x_error("Cannot read the keyboard map", e))?
            .reply()
            .map_err(|e| x_error("Cannot read the keyboard map", e))?;
        Ok(Display {
            connection,
            root,
            first_keycode: first,
            per_key: usize::from(map.keysyms_per_keycode),
            symbols: map.keysyms,
        })
    }

    fn fake(&self, kind: u8, detail: u8, x: i16, y: i16) -> Result<(), AppError> {
        self.connection
            .xtest_fake_input(kind, detail, 0, self.root, x, y, 0)
            .map_err(|e| x_error("Cannot send input", e))?;
        Ok(())
    }

    fn finish(&self) -> Result<(), AppError> {
        self.connection
            .sync()
            .map_err(|e| x_error("Cannot flush input", e))
    }

    fn keycode(&self, keysym: u32) -> Result<(u8, bool), AppError> {
        find_key(&self.symbols, self.per_key, self.first_keycode, keysym)
            .map(|place| (place.keycode, place.needs_shift))
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::ActionNotSupported,
                    "This key is not on the current keyboard layout",
                )
            })
    }

    fn press(&self, combo: &Combo) -> Result<(), AppError> {
        let mut held: Vec<u8> = Vec::new();
        for modifier in &combo.modifiers {
            held.push(self.keycode(modifier_keysym(*modifier))?.0);
        }
        let (key, needs_shift) = self.keycode(keysym_for(combo.key))?;
        if needs_shift && !held.contains(&self.keycode(SHIFT_KEYSYM)?.0) {
            held.push(self.keycode(SHIFT_KEYSYM)?.0);
        }
        for code in &held {
            self.fake(KEY_PRESS_EVENT, *code, 0, 0)?;
        }
        self.fake(KEY_PRESS_EVENT, key, 0, 0)?;
        self.fake(KEY_RELEASE_EVENT, key, 0, 0)?;
        for code in held.iter().rev() {
            self.fake(KEY_RELEASE_EVENT, *code, 0, 0)?;
        }
        self.finish()
    }

    fn type_text(&self, text: &str) -> Result<(), AppError> {
        let mut plan = Vec::new();
        for character in text.chars() {
            let keysym = char_keysym(character).ok_or_else(|| {
                AppError::new(
                    ErrorCode::ActionNotSupported,
                    format!("'{character}' cannot be typed with keys (letters, digits and common symbols only)"),
                )
                .with_suggestion("Use set-value on the element for accents and other alphabets")
            })?;
            plan.push(self.keycode(keysym)?);
        }
        let shift = self.keycode(SHIFT_KEYSYM)?.0;
        for (key, needs_shift) in plan {
            if needs_shift {
                self.fake(KEY_PRESS_EVENT, shift, 0, 0)?;
            }
            self.fake(KEY_PRESS_EVENT, key, 0, 0)?;
            self.fake(KEY_RELEASE_EVENT, key, 0, 0)?;
            if needs_shift {
                self.fake(KEY_RELEASE_EVENT, shift, 0, 0)?;
            }
            self.finish()?;
        }
        Ok(())
    }

    fn move_to(&self, x: f64, y: f64) -> Result<(), AppError> {
        self.fake(MOTION_NOTIFY_EVENT, 0, x.round() as i16, y.round() as i16)
    }

    fn click(&self, button: u8, count: u32) -> Result<(), AppError> {
        for _ in 0..count {
            self.fake(BUTTON_PRESS_EVENT, button, 0, 0)?;
            self.fake(BUTTON_RELEASE_EVENT, button, 0, 0)?;
            self.finish()?;
            std::thread::sleep(PAUSE);
        }
        Ok(())
    }
}

fn route() -> Result<KeyRoute, AppError> {
    let session = std::env::var("XDG_SESSION_TYPE").ok();
    key_route(
        session.as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    )
    .map_err(|reason| AppError::new(ErrorCode::PlatformNotSupported, reason))
}

pub fn press(combo: &Combo) -> Result<(), AppError> {
    match route()? {
        KeyRoute::Wayland => crate::wayland_keys::press(combo),
        KeyRoute::X11 => Display::open()?.press(combo),
    }
}

pub fn type_text(text: &str) -> Result<(), AppError> {
    match route()? {
        KeyRoute::Wayland => crate::wayland_keys::type_text(text),
        KeyRoute::X11 => Display::open()?.type_text(text),
    }
}

pub fn mouse_move(x: f64, y: f64) -> Result<(), AppError> {
    let display = Display::open()?;
    display.move_to(x, y)?;
    display.finish()
}

pub fn mouse_click(x: f64, y: f64, button: MouseButton, count: u8) -> Result<(), AppError> {
    let display = Display::open()?;
    display.move_to(x, y)?;
    display.finish()?;
    let number = match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
    };
    display.click(number, u32::from(count))
}

pub fn mouse_wheel(dx: i32, dy: i32) -> Result<(), AppError> {
    let display = Display::open()?;
    if dy != 0 {
        display.click(if dy > 0 { 5 } else { 4 }, dy.unsigned_abs())?;
    }
    if dx != 0 {
        display.click(if dx > 0 { 7 } else { 6 }, dx.unsigned_abs())?;
    }
    Ok(())
}
