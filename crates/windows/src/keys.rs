//! Windows virtual-key codes for the shared key vocabulary (US keyboard layout).
//! Pure functions, so they are tested on every OS.
#![cfg_attr(not(windows), allow(dead_code))]

use agentic_desktop_core::input::{Key, Modifier, NamedKey};

#[derive(Debug, PartialEq, Eq)]
pub struct VirtualKey {
    pub code: u16,
    /// Arrow, Home, End, Page and Delete keys need the "extended key" flag.
    pub extended: bool,
    pub needs_shift: bool,
}

fn plain(code: u16) -> Option<VirtualKey> {
    Some(VirtualKey {
        code,
        extended: false,
        needs_shift: false,
    })
}

fn extended(code: u16) -> Option<VirtualKey> {
    Some(VirtualKey {
        code,
        extended: true,
        needs_shift: false,
    })
}

pub fn modifier_code(modifier: Modifier) -> u16 {
    match modifier {
        Modifier::Ctrl => 0x11,
        Modifier::Shift => 0x10,
        Modifier::Alt => 0x12,
        Modifier::Meta => 0x5B,
    }
}

pub fn virtual_key(key: Key) -> Option<VirtualKey> {
    match key {
        Key::Char(c) if c.is_ascii_lowercase() => plain(u16::from(c as u8 - b'a') + 0x41),
        Key::Char(c) if c.is_ascii_digit() => plain(u16::from(c as u8)),
        Key::Char('+') => Some(VirtualKey {
            code: 0xBB,
            extended: false,
            needs_shift: true,
        }),
        Key::Char(c) => match c {
            '-' => plain(0xBD),
            '=' => plain(0xBB),
            '[' => plain(0xDB),
            ']' => plain(0xDD),
            '\\' => plain(0xDC),
            ';' => plain(0xBA),
            '\'' => plain(0xDE),
            ',' => plain(0xBC),
            '.' => plain(0xBE),
            '/' => plain(0xBF),
            '`' => plain(0xC0),
            _ => None,
        },
        Key::Named(named) => match named {
            NamedKey::Enter => plain(0x0D),
            NamedKey::Tab => plain(0x09),
            NamedKey::Escape => plain(0x1B),
            NamedKey::Space => plain(0x20),
            NamedKey::Backspace => plain(0x08),
            NamedKey::Delete => extended(0x2E),
            NamedKey::Left => extended(0x25),
            NamedKey::Up => extended(0x26),
            NamedKey::Right => extended(0x27),
            NamedKey::Down => extended(0x28),
            NamedKey::Home => extended(0x24),
            NamedKey::End => extended(0x23),
            NamedKey::PageUp => extended(0x21),
            NamedKey::PageDown => extended(0x22),
            NamedKey::F(n) if (1..=12).contains(&n) => plain(0x70 + u16::from(n) - 1),
            NamedKey::F(_) => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_function_keys() {
        assert_eq!(virtual_key(Key::Char('a')).unwrap().code, 0x41);
        assert_eq!(virtual_key(Key::Char('z')).unwrap().code, 0x5A);
        assert_eq!(virtual_key(Key::Char('0')).unwrap().code, 0x30);
        assert_eq!(virtual_key(Key::Named(NamedKey::F(1))).unwrap().code, 0x70);
        assert_eq!(virtual_key(Key::Named(NamedKey::F(12))).unwrap().code, 0x7B);
    }

    #[test]
    fn navigation_keys_are_extended() {
        assert!(virtual_key(Key::Named(NamedKey::Left)).unwrap().extended);
        assert!(virtual_key(Key::Named(NamedKey::Delete)).unwrap().extended);
        assert!(!virtual_key(Key::Named(NamedKey::Enter)).unwrap().extended);
    }

    #[test]
    fn plus_needs_shift_and_unknown_keys_are_none() {
        let plus = virtual_key(Key::Char('+')).unwrap();
        assert_eq!((plus.code, plus.needs_shift), (0xBB, true));
        assert_eq!(virtual_key(Key::Char('~')), None);
    }

    #[test]
    fn modifiers() {
        assert_eq!(modifier_code(Modifier::Ctrl), 0x11);
        assert_eq!(modifier_code(Modifier::Meta), 0x5B);
    }
}
