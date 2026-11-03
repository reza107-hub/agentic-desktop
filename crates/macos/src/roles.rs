//! Map macOS Accessibility (AX) names to the shared vocabulary.
//! Pure functions, so they are tested on every OS.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

pub fn canonical_role(ax_role: &str, ax_subrole: Option<&str>) -> String {
    let role = match (ax_role, ax_subrole) {
        ("AXButton", Some("AXToggleButton")) => "switch",
        ("AXButton" | "AXMenuButton", _) => "button",
        ("AXCheckBox", Some("AXSwitch")) => "switch",
        ("AXCheckBox", _) => "checkbox",
        ("AXRadioButton", _) => "radiobutton",
        ("AXTextField" | "AXTextArea" | "AXSearchField" | "AXSecureTextField", _) => "textfield",
        ("AXComboBox" | "AXPopUpButton", _) => "combobox",
        ("AXMenuItem" | "AXMenuBarItem", _) => "menuitem",
        ("AXLink", _) => "link",
        ("AXSlider", _) => "slider",
        ("AXStaticText", _) => "statictext",
        ("AXWindow" | "AXSheet" | "AXDrawer", _) => "window",
        ("AXApplication", _) => "application",
        ("AXGroup" | "AXSplitGroup" | "AXScrollArea" | "AXLayoutArea", _) => "group",
        ("AXTabGroup", _) => "tablist",
        ("AXToolbar", _) => "toolbar",
        ("AXOutline", _) => "tree",
        ("AXRow", _) => "listitem",
        (other, _) => return other.strip_prefix("AX").unwrap_or(other).to_lowercase(),
    };
    role.to_string()
}

/// `AXPress` becomes `press`, `AXShowMenu` becomes `showmenu`.
pub fn action_name(ax_action: &str) -> String {
    ax_action
        .strip_prefix("AX")
        .unwrap_or(ax_action)
        .to_lowercase()
}

/// Actions that exist on almost every element and say nothing useful on their own.
pub fn is_noise_action(name: &str) -> bool {
    matches!(
        name,
        "showmenu" | "scrolltovisible" | "showdefaultui" | "showalternateui"
    )
}

/// The app name agents use: the first `Name.app` folder in the program path.
/// Helper apps nested inside another app (`Foo.app/.../Helper.app`) are not apps.
pub fn app_name_from_path(path: &str) -> Option<String> {
    if !path.contains(".app/Contents/MacOS/") {
        return None;
    }
    let first = path.split(".app/").next()?;
    if path.matches(".app/").count() > 1 {
        return None;
    }
    let name = first.rsplit('/').next()?;
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_roles() {
        assert_eq!(canonical_role("AXButton", None), "button");
        assert_eq!(canonical_role("AXCheckBox", Some("AXSwitch")), "switch");
        assert_eq!(canonical_role("AXSearchField", None), "textfield");
        assert_eq!(canonical_role("AXTable", None), "table");
    }

    #[test]
    fn maps_actions() {
        assert_eq!(action_name("AXPress"), "press");
        assert!(is_noise_action(&action_name("AXShowMenu")));
        assert!(!is_noise_action("press"));
    }

    #[test]
    fn app_names_come_from_the_first_app_folder() {
        assert_eq!(
            app_name_from_path("/Applications/Safari.app/Contents/MacOS/Safari").as_deref(),
            Some("Safari")
        );
        assert_eq!(
            app_name_from_path(
                "/Applications/Google Chrome.app/Contents/Frameworks/X.framework/Helper.app/Contents/MacOS/Helper"
            ),
            None
        );
        assert_eq!(app_name_from_path("/usr/bin/ssh"), None);
    }
}
