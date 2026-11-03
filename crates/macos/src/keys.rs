//! macOS key codes for the shared key vocabulary (US ANSI layout).
//! Pure functions, so they are tested on every OS.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use agentic_desktop_core::input::{Key, Modifier, NamedKey};

pub const FLAG_SHIFT: u64 = 0x0002_0000;
pub const FLAG_CONTROL: u64 = 0x0004_0000;
pub const FLAG_OPTION: u64 = 0x0008_0000;
pub const FLAG_COMMAND: u64 = 0x0010_0000;

#[derive(Debug, PartialEq, Eq)]
pub struct MacKey {
    pub code: u16,
    pub needs_shift: bool,
}

pub fn modifier_flag(modifier: Modifier) -> u64 {
    match modifier {
        Modifier::Ctrl => FLAG_CONTROL,
        Modifier::Shift => FLAG_SHIFT,
        Modifier::Alt => FLAG_OPTION,
        Modifier::Meta => FLAG_COMMAND,
    }
}

fn plain(code: u16) -> Option<MacKey> {
    Some(MacKey {
        code,
        needs_shift: false,
    })
}

pub fn mac_key(key: Key) -> Option<MacKey> {
    match key {
        Key::Char('+') => Some(MacKey {
            code: 24,
            needs_shift: true,
        }),
        Key::Char(c) => plain(match c {
            'a' => 0,
            's' => 1,
            'd' => 2,
            'f' => 3,
            'h' => 4,
            'g' => 5,
            'z' => 6,
            'x' => 7,
            'c' => 8,
            'v' => 9,
            'b' => 11,
            'q' => 12,
            'w' => 13,
            'e' => 14,
            'r' => 15,
            'y' => 16,
            't' => 17,
            '1' => 18,
            '2' => 19,
            '3' => 20,
            '4' => 21,
            '6' => 22,
            '5' => 23,
            '=' => 24,
            '9' => 25,
            '7' => 26,
            '-' => 27,
            '8' => 28,
            '0' => 29,
            ']' => 30,
            'o' => 31,
            'u' => 32,
            '[' => 33,
            'i' => 34,
            'p' => 35,
            'l' => 37,
            'j' => 38,
            '\'' => 39,
            'k' => 40,
            ';' => 41,
            '\\' => 42,
            ',' => 43,
            '/' => 44,
            'n' => 45,
            'm' => 46,
            '.' => 47,
            '`' => 50,
            _ => return None,
        }),
        Key::Named(named) => plain(match named {
            NamedKey::Enter => 36,
            NamedKey::Tab => 48,
            NamedKey::Space => 49,
            NamedKey::Backspace => 51,
            NamedKey::Escape => 53,
            NamedKey::Delete => 117,
            NamedKey::Home => 115,
            NamedKey::End => 119,
            NamedKey::PageUp => 116,
            NamedKey::PageDown => 121,
            NamedKey::Left => 123,
            NamedKey::Right => 124,
            NamedKey::Down => 125,
            NamedKey::Up => 126,
            NamedKey::F(n) => match n {
                1 => 122,
                2 => 120,
                3 => 99,
                4 => 118,
                5 => 96,
                6 => 97,
                7 => 98,
                8 => 100,
                9 => 101,
                10 => 109,
                11 => 103,
                12 => 111,
                _ => return None,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn known_codes() {
        assert_eq!(mac_key(Key::Char('a')).unwrap().code, 0);
        assert_eq!(mac_key(Key::Char('c')).unwrap().code, 8);
        assert_eq!(mac_key(Key::Named(NamedKey::Enter)).unwrap().code, 36);
        assert_eq!(mac_key(Key::Named(NamedKey::F(1))).unwrap().code, 122);
        assert_eq!(mac_key(Key::Named(NamedKey::Up)).unwrap().code, 126);
    }

    #[test]
    fn every_supported_key_has_a_distinct_code() {
        let mut keys: Vec<Key> = ('a'..='z').chain('0'..='9').map(Key::Char).collect();
        keys.extend(r"-=[]\;',./`".chars().map(Key::Char));
        keys.extend((1..=12).map(|n| Key::Named(NamedKey::F(n))));
        let mut seen = HashSet::new();
        for key in keys {
            let code = mac_key(key)
                .unwrap_or_else(|| panic!("{key:?} has no code"))
                .code;
            assert!(seen.insert(code), "{key:?} repeats code {code}");
        }
    }

    #[test]
    fn plus_needs_shift() {
        assert_eq!(
            mac_key(Key::Char('+')),
            Some(MacKey {
                code: 24,
                needs_shift: true
            })
        );
    }

    #[test]
    fn modifier_flags_do_not_overlap() {
        let all = [
            Modifier::Ctrl,
            Modifier::Shift,
            Modifier::Alt,
            Modifier::Meta,
        ]
        .map(modifier_flag);
        assert_eq!(all.iter().fold(0, |sum, flag| sum | flag).count_ones(), 4);
    }
}
