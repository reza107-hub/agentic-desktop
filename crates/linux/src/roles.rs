//! Turn AT-SPI role names and state bits into the shared vocabulary.

pub const STATE_EDITABLE: u32 = 7;

/// AT-SPI sends states as two 32-bit words; bit `n` lives in word `n / 32`.
pub fn has_state(words: &[u32], state: u32) -> bool {
    words
        .get((state / 32) as usize)
        .is_some_and(|word| word & (1 << (state % 32)) != 0)
}

pub fn canonical_role(role_name: &str, editable: bool) -> String {
    let role = match role_name {
        "push button" | "toggle button" | "button" => "button",
        "check box" | "check menu item" => "checkbox",
        "radio button" | "radio menu item" => "radiobutton",
        "menu item" => "menuitem",
        "page tab" => "tab",
        "combo box" => "combobox",
        "entry" | "textbox" | "text box" | "password text" | "spin button" => "textfield",
        "text" if editable => "textfield",
        "text" | "label" | "static" | "heading" | "paragraph" => "statictext",
        "link" => "link",
        "slider" => "slider",
        "switch" => "switch",
        "list item" => "listitem",
        "tree item" => "treeitem",
        "frame" | "window" | "dialog" | "alert" | "file chooser" => "window",
        "application" => "application",
        "generic" | "panel" | "filler" | "section" | "viewport" | "scroll pane" => "group",
        other => return other.replace([' ', '-'], "").to_lowercase(),
    };
    role.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_roles() {
        assert_eq!(canonical_role("push button", false), "button");
        assert_eq!(canonical_role("page tab", false), "tab");
        assert_eq!(canonical_role("text", true), "textfield");
        assert_eq!(canonical_role("text", false), "statictext");
        assert_eq!(canonical_role("table cell", false), "tablecell");
        assert_eq!(canonical_role("textbox", false), "textfield");
        assert_eq!(canonical_role("text box", false), "textfield");
        assert_eq!(canonical_role("generic", false), "group");
    }

    #[test]
    fn reads_state_bits_across_both_words() {
        assert!(has_state(&[1 << STATE_EDITABLE, 0], STATE_EDITABLE));
        assert!(!has_state(&[0, 0], STATE_EDITABLE));
        assert!(has_state(&[0, 1 << 2], 34));
        assert!(!has_state(&[], 6));
    }
}
