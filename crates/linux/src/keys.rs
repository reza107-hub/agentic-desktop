//! X11 key symbols for the shared key vocabulary, and the rules for when input is allowed.
//! Pure functions, so they are tested without a display.

use agentic_desktop_core::input::{Key, Modifier, NamedKey};

pub fn modifier_keysym(modifier: Modifier) -> u32 {
    match modifier {
        Modifier::Ctrl => 0xffe3,
        Modifier::Shift => 0xffe1,
        Modifier::Alt => 0xffe9,
        Modifier::Meta => 0xffeb,
    }
}

pub fn keysym_for(key: Key) -> u32 {
    match key {
        Key::Char(c) => c as u32,
        Key::Named(named) => match named {
            NamedKey::Enter => 0xff0d,
            NamedKey::Tab => 0xff09,
            NamedKey::Escape => 0xff1b,
            NamedKey::Space => 0x20,
            NamedKey::Backspace => 0xff08,
            NamedKey::Delete => 0xffff,
            NamedKey::Home => 0xff50,
            NamedKey::Left => 0xff51,
            NamedKey::Up => 0xff52,
            NamedKey::Right => 0xff53,
            NamedKey::Down => 0xff54,
            NamedKey::PageUp => 0xff55,
            NamedKey::PageDown => 0xff56,
            NamedKey::End => 0xff57,
            NamedKey::F(n) => 0xffbe + u32::from(n.clamp(1, 12)) - 1,
        },
    }
}

/// The key symbol for a character that can be typed with keys: printable ASCII.
pub fn char_keysym(character: char) -> Option<u32> {
    (' '..='~').contains(&character).then_some(character as u32)
}

/// Which way keys are sent in this session.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KeyRoute {
    /// GNOME Wayland, through Mutter's RemoteDesktop service.
    Wayland,
    /// X11, through the XTEST extension.
    X11,
}

/// Choose the key route. A Wayland session always uses Wayland keys, even though
/// an X11 display also exists there, because X11 input would only reach old apps.
pub fn key_route(
    session_type: Option<&str>,
    wayland_display_set: bool,
    x_display_set: bool,
) -> Result<KeyRoute, &'static str> {
    if session_type == Some("wayland") || wayland_display_set {
        return Ok(KeyRoute::Wayland);
    }
    if x_display_set {
        return Ok(KeyRoute::X11);
    }
    Err("No display is available for keyboard input (neither WAYLAND_DISPLAY nor DISPLAY is set)")
}

/// Where a key symbol sits in the keyboard map: plain, or only with Shift held.
#[derive(Debug, PartialEq, Eq)]
pub struct KeyPlace {
    pub keycode: u8,
    pub needs_shift: bool,
}

/// Find `keysym` in a keyboard map. `symbols` holds `per_key` symbols for each key,
/// starting at `first_keycode`. Only columns 0 (plain) and 1 (Shift) are used.
pub fn find_key(
    symbols: &[u32],
    per_key: usize,
    first_keycode: u8,
    keysym: u32,
) -> Option<KeyPlace> {
    if per_key == 0 {
        return None;
    }
    let columns = per_key.min(2);
    for column in 0..columns {
        for (index, row) in symbols.chunks(per_key).enumerate() {
            if row.get(column) == Some(&keysym) {
                let keycode = u8::try_from(usize::from(first_keycode) + index).ok()?;
                return Some(KeyPlace {
                    keycode,
                    needs_shift: column == 1,
                });
            }
        }
    }
    None
}

/// Why the pointer cannot be moved or clicked here, or None when it can.
/// Wayland gives programs no way to aim the pointer, and X11 test input there only
/// reaches old X11 apps, which would mislead the caller. Keys are a separate route.
pub fn input_block_reason(
    session_type: Option<&str>,
    wayland_display_set: bool,
    x_display_set: bool,
) -> Option<&'static str> {
    if session_type == Some("wayland") || wayland_display_set {
        return Some(
            "Wayland does not let programs aim or click the pointer. Keys still work (press, type-text); for clicking, use click on an element, or log in to an X11 session",
        );
    }
    if !x_display_set {
        return Some("No X11 display is available (DISPLAY is not set)");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_named_keys_have_symbols() {
        assert_eq!(keysym_for(Key::Char('a')), 0x61);
        assert_eq!(keysym_for(Key::Named(NamedKey::Enter)), 0xff0d);
        assert_eq!(keysym_for(Key::Named(NamedKey::F(1))), 0xffbe);
        assert_eq!(keysym_for(Key::Named(NamedKey::F(12))), 0xffc9);
    }

    #[test]
    fn finds_plain_and_shifted_keys() {
        let map = [0x61, 0x41, 0x3d, 0x2b, 0x31, 0x21];
        assert_eq!(
            find_key(&map, 2, 38, 0x61),
            Some(KeyPlace {
                keycode: 38,
                needs_shift: false
            })
        );
        assert_eq!(
            find_key(&map, 2, 38, 0x2b),
            Some(KeyPlace {
                keycode: 39,
                needs_shift: true
            })
        );
        assert_eq!(
            find_key(&map, 2, 38, 0x31),
            Some(KeyPlace {
                keycode: 40,
                needs_shift: false
            })
        );
        assert_eq!(find_key(&map, 2, 38, 0x7a), None);
        assert_eq!(find_key(&map, 0, 38, 0x61), None);
    }

    #[test]
    fn keys_go_by_session_type() {
        assert_eq!(
            key_route(Some("wayland"), true, true),
            Ok(KeyRoute::Wayland)
        );
        assert_eq!(key_route(None, true, true), Ok(KeyRoute::Wayland));
        assert_eq!(key_route(Some("x11"), false, true), Ok(KeyRoute::X11));
        assert!(key_route(None, false, false).is_err());
    }

    #[test]
    fn printable_ascii_has_key_symbols() {
        assert_eq!(char_keysym('a'), Some(0x61));
        assert_eq!(char_keysym(' '), Some(0x20));
        assert_eq!(char_keysym('~'), Some(0x7e));
        assert_eq!(char_keysym('\u{e9}'), None);
        assert_eq!(char_keysym('\n'), None);
    }

    #[test]
    fn wayland_blocks_input() {
        assert!(input_block_reason(Some("wayland"), true, true).is_some());
        assert!(input_block_reason(None, true, true).is_some());
        assert!(input_block_reason(Some("x11"), false, false).is_some());
        assert!(input_block_reason(Some("x11"), false, true).is_none());
        assert!(input_block_reason(None, false, true).is_none());
    }
}
