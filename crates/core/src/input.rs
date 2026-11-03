//! Keyboard and mouse vocabulary shared by every backend.
//! Parsing is pure, so it is tested on every OS.

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    /// Command on macOS, Windows key on Windows, Super on Linux.
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    Enter,
    Tab,
    Escape,
    Space,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// Function key 1 to 12.
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A lowercase letter, digit or one of ``-=[]\;',./` ``.
    Char(char),
    Named(NamedKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combo {
    pub modifiers: Vec<Modifier>,
    pub key: Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn parse(text: &str) -> Result<Self, AppError> {
        match text.to_ascii_lowercase().as_str() {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "middle" => Ok(Self::Middle),
            other => Err(
                AppError::invalid_args(format!("'{other}' is not a mouse button"))
                    .with_suggestion("Use left, right or middle"),
            ),
        }
    }
}

const PUNCTUATION: &str = r"-=[]\;',./`";

fn parse_modifier(token: &str) -> Option<Modifier> {
    match token {
        "ctrl" | "control" => Some(Modifier::Ctrl),
        "shift" => Some(Modifier::Shift),
        "alt" | "option" => Some(Modifier::Alt),
        "meta" | "cmd" | "command" | "super" | "win" | "windows" => Some(Modifier::Meta),
        _ => None,
    }
}

fn parse_key(token: &str) -> Option<Key> {
    let named = match token {
        "enter" | "return" => Some(NamedKey::Enter),
        "tab" => Some(NamedKey::Tab),
        "escape" | "esc" => Some(NamedKey::Escape),
        "space" => Some(NamedKey::Space),
        "backspace" => Some(NamedKey::Backspace),
        "delete" | "del" => Some(NamedKey::Delete),
        "up" => Some(NamedKey::Up),
        "down" => Some(NamedKey::Down),
        "left" => Some(NamedKey::Left),
        "right" => Some(NamedKey::Right),
        "home" => Some(NamedKey::Home),
        "end" => Some(NamedKey::End),
        "pageup" => Some(NamedKey::PageUp),
        "pagedown" => Some(NamedKey::PageDown),
        _ => None,
    };
    if let Some(named) = named {
        return Some(Key::Named(named));
    }
    if token == "plus" {
        return Some(Key::Char('+'));
    }
    if let Some(number) = token.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
        && (1..=12).contains(&number)
    {
        return Some(Key::Named(NamedKey::F(number)));
    }
    let mut chars = token.chars();
    let (Some(only), None) = (chars.next(), chars.next()) else {
        return None;
    };
    (only.is_ascii_lowercase() || only.is_ascii_digit() || PUNCTUATION.contains(only))
        .then_some(Key::Char(only))
}

/// Parse `ctrl+shift+a`, `enter`, `alt+f4` or `ctrl+plus`.
pub fn parse_combo(text: &str) -> Result<Combo, AppError> {
    let bad = |why: String| {
        AppError::invalid_args(why)
            .with_suggestion("Examples: enter, ctrl+c, ctrl+shift+t, alt+f4, ctrl+plus")
    };
    let lowered = text.trim().to_lowercase();
    if lowered.is_empty() {
        return Err(bad("The key combination is empty".to_string()));
    }
    let parts: Vec<&str> = lowered.split('+').collect();
    let (key_token, modifier_tokens) =
        parts.split_last().ok_or_else(|| bad("Empty".to_string()))?;
    let mut modifiers = Vec::new();
    for token in modifier_tokens {
        let modifier = parse_modifier(token)
            .ok_or_else(|| bad(format!("'{token}' is not a modifier in '{text}'")))?;
        if modifiers.contains(&modifier) {
            return Err(bad(format!("'{token}' is repeated in '{text}'")));
        }
        modifiers.push(modifier);
    }
    let key = parse_key(key_token)
        .ok_or_else(|| bad(format!("'{key_token}' is not a known key in '{text}'")))?;
    Ok(Combo { modifiers, key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_named_keys() {
        assert_eq!(parse_combo("a").unwrap().key, Key::Char('a'));
        assert_eq!(
            parse_combo("Enter").unwrap().key,
            Key::Named(NamedKey::Enter)
        );
        assert_eq!(parse_combo("f12").unwrap().key, Key::Named(NamedKey::F(12)));
    }

    #[test]
    fn parses_modifiers_in_any_case() {
        let combo = parse_combo("Ctrl+Shift+T").unwrap();
        assert_eq!(combo.modifiers, vec![Modifier::Ctrl, Modifier::Shift]);
        assert_eq!(combo.key, Key::Char('t'));
        assert_eq!(
            parse_combo("cmd+space").unwrap().modifiers,
            vec![Modifier::Meta]
        );
    }

    #[test]
    fn plus_key_has_a_name() {
        assert_eq!(parse_combo("ctrl+plus").unwrap().key, Key::Char('+'));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            "",
            "+",
            "ctrl+",
            "ctrl+ctrl+a",
            "banana+a",
            "f13",
            "f0",
            "ab",
            "ctrl+A+b",
            "ä",
        ] {
            assert!(parse_combo(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn mouse_buttons_parse() {
        assert_eq!(MouseButton::parse("Right").unwrap(), MouseButton::Right);
        assert!(MouseButton::parse("fourth").is_err());
    }
}
