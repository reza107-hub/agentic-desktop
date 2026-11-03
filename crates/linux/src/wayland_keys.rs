//! Real key presses on GNOME Wayland, through Mutter's RemoteDesktop service.
//! The service takes key symbols, so no keyboard layout has to be read.
//!
//! Steps: check the service exists, create a session, start it, tap Shift once so
//! Mutter wakes its virtual keyboard, send the keys, then stop the session.

use crate::keys::{char_keysym, keysym_for, modifier_keysym};
use agentic_desktop_core::input::Combo;
use agentic_desktop_core::{AppError, ErrorCode};
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

const SERVICE: &str = "org.gnome.Mutter.RemoteDesktop";
const PATH: &str = "/org/gnome/Mutter/RemoteDesktop";
const SESSION: &str = "org.gnome.Mutter.RemoteDesktop.Session";
const SHIFT_LEFT: u32 = 0xffe1;
const CALL_TIMEOUT: Duration = Duration::from_secs(3);
const WAKE_PAUSE: Duration = Duration::from_millis(100);
/// Gap between key events, so a slow app does not drop or reorder them.
const KEY_PAUSE: Duration = Duration::from_millis(8);

/// One key event: a key symbol going down or up.
pub type Step = (u32, bool);

/// The events for a shortcut: modifiers down, the key down and up, modifiers up.
pub fn steps_for_combo(combo: &Combo) -> Vec<Step> {
    let modifiers: Vec<u32> = combo
        .modifiers
        .iter()
        .map(|m| modifier_keysym(*m))
        .collect();
    let key = keysym_for(combo.key);
    let mut steps: Vec<Step> = modifiers.iter().map(|sym| (*sym, true)).collect();
    steps.push((key, true));
    steps.push((key, false));
    steps.extend(modifiers.iter().rev().map(|sym| (*sym, false)));
    steps
}

/// The events for typing text. Refuses the whole text, before any key is sent,
/// when a character has no key symbol this route can send.
pub fn steps_for_text(text: &str) -> Result<Vec<Step>, AppError> {
    let mut steps = Vec::with_capacity(text.len() * 2);
    for character in text.chars() {
        let keysym = char_keysym(character).ok_or_else(|| {
            AppError::new(
                ErrorCode::ActionNotSupported,
                format!("'{character}' cannot be typed with keys on Wayland (letters, digits and common symbols only)"),
            )
            .with_suggestion("Use set-value on the element for accents and other alphabets")
        })?;
        steps.push((keysym, true));
        steps.push((keysym, false));
    }
    Ok(steps)
}

fn classify(error: &zbus::Error) -> AppError {
    let text = error.to_string();
    let lowered = text.to_lowercase();
    if lowered.contains("timed out") || lowered.contains("timeout") {
        return AppError::new(
            ErrorCode::Timeout,
            "The desktop did not answer the key request in time",
        );
    }
    if ["accessdenied", "serviceunknown", "name has no owner"]
        .iter()
        .any(|word| lowered.contains(word))
    {
        return AppError::new(
            ErrorCode::ActionNotSupported,
            format!("Keyboard input through the desktop is not available in this session ({text})"),
        )
        .with_suggestion("Use click, type and set-value on elements instead");
    }
    AppError::new(
        ErrorCode::ActionFailed,
        format!("The desktop rejected the key request: {text}"),
    )
}

fn connect() -> Result<Connection, AppError> {
    let connection = Builder::session()
        .and_then(|builder| builder.method_timeout(CALL_TIMEOUT).build())
        .map_err(|e| classify(&e))?;
    connection
        .call_method(
            Some(SERVICE),
            PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(SERVICE, "Version"),
        )
        .map_err(|e| {
            AppError::new(
                ErrorCode::ActionNotSupported,
                format!("This desktop does not offer Mutter RemoteDesktop keyboard input ({e})"),
            )
            .with_suggestion("Use click, type and set-value on elements instead")
        })?;
    Ok(connection)
}

fn call(
    connection: &Connection,
    path: &str,
    member: &str,
    body: &(impl serde::Serialize + zbus::zvariant::DynamicType),
) -> Result<(), AppError> {
    connection
        .call_method(Some(SERVICE), path, Some(SESSION), member, body)
        .map(|_| ())
        .map_err(|e| classify(&e))
}

/// Send key events. Keys still held when something fails are released first.
pub fn send(steps: &[Step]) -> Result<(), AppError> {
    if steps.is_empty() {
        return Ok(());
    }
    let connection = connect()?;
    let created = connection
        .call_method(Some(SERVICE), PATH, Some(SERVICE), "CreateSession", &())
        .map_err(|e| classify(&e))?;
    let session: OwnedObjectPath = created
        .body()
        .deserialize()
        .map_err(|e| AppError::internal(format!("Bad reply to CreateSession: {e}")))?;
    let path = session.as_str();

    let outcome = (|| {
        call(&connection, path, "Start", &())?;
        call(
            &connection,
            path,
            "NotifyKeyboardKeysym",
            &(SHIFT_LEFT, true),
        )?;
        call(
            &connection,
            path,
            "NotifyKeyboardKeysym",
            &(SHIFT_LEFT, false),
        )?;
        std::thread::sleep(WAKE_PAUSE);
        let mut held: Vec<u32> = Vec::new();
        for (keysym, down) in steps {
            if let Err(error) = call(&connection, path, "NotifyKeyboardKeysym", &(*keysym, *down)) {
                for stuck in held.iter().rev() {
                    let _ = call(&connection, path, "NotifyKeyboardKeysym", &(*stuck, false));
                }
                return Err(error);
            }
            if *down {
                held.push(*keysym);
            } else if let Some(at) = held.iter().rposition(|sym| sym == keysym) {
                held.remove(at);
            }
            std::thread::sleep(KEY_PAUSE);
        }
        Ok(())
    })();
    let _ = call(&connection, path, "Stop", &());
    outcome
}

pub fn press(combo: &Combo) -> Result<(), AppError> {
    send(&steps_for_combo(combo))
}

pub fn type_text(text: &str) -> Result<(), AppError> {
    send(&steps_for_text(text)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentic_desktop_core::input::parse_combo;

    #[test]
    fn a_shortcut_presses_modifiers_around_the_key() {
        let steps = steps_for_combo(&parse_combo("ctrl+shift+t").unwrap());
        assert_eq!(
            steps,
            vec![
                (0xffe3, true),
                (0xffe1, true),
                (0x74, true),
                (0x74, false),
                (0xffe1, false),
                (0xffe3, false),
            ]
        );
    }

    #[test]
    fn a_bare_key_is_down_then_up() {
        assert_eq!(
            steps_for_combo(&parse_combo("enter").unwrap()),
            vec![(0xff0d, true), (0xff0d, false)]
        );
    }

    #[test]
    fn text_becomes_key_pairs() {
        assert_eq!(
            steps_for_text("a.B").unwrap(),
            vec![
                (0x61, true),
                (0x61, false),
                (0x2e, true),
                (0x2e, false),
                (0x42, true),
                (0x42, false)
            ]
        );
    }

    #[test]
    fn text_with_a_character_outside_ascii_is_refused_whole() {
        let error = steps_for_text("caf\u{e9}").unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert!(error.message.contains('\u{e9}'));
    }

    /// Opens a real session and taps Shift once, which has no visible effect.
    /// Run with: cargo test -p agentic-desktop-linux -- --ignored --nocapture mutter_session
    #[test]
    #[ignore = "needs a live GNOME Wayland session"]
    fn mutter_session_accepts_a_shift_tap() {
        let result = send(&[(SHIFT_LEFT, true), (SHIFT_LEFT, false)]);
        println!("MUTTER RESULT: {result:?}");
        result.expect("the session should accept a Shift tap");
    }
}
