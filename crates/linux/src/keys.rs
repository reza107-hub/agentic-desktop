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

/// Why synthetic input cannot work here, or None when it can.
/// On Wayland, X11 test input only reaches old X11 apps, which would mislead the caller.
pub fn input_block_reason(
    session_type: Option<&str>,
    wayland_display_set: bool,
    x_display_set: bool,
) -> Option<&'static str> {
    if session_type == Some("wayland") || wayland_display_set {
        return Some(
            "Wayland blocks synthetic keyboard and mouse input. Log in to an X11 session, or use ref actions (click, type, set-value) instead",
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
    fn wayland_blocks_input() {
        assert!(input_block_reason(Some("wayland"), true, true).is_some());
        assert!(input_block_reason(None, true, true).is_some());
        assert!(input_block_reason(Some("x11"), false, false).is_some());
        assert!(input_block_reason(Some("x11"), false, true).is_none());
        assert!(input_block_reason(None, false, true).is_none());
    }
}
