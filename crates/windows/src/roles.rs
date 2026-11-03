//! Map UI Automation control type ids to the shared role vocabulary.
//! Pure functions, so they are tested on every OS.
#![cfg_attr(not(windows), allow(dead_code))]

pub fn canonical_role(control_type: i32) -> &'static str {
    match control_type {
        50000 | 50031 => "button",
        50001 => "calendar",
        50002 => "checkbox",
        50003 => "combobox",
        50004 => "textfield",
        50005 => "link",
        50006 => "image",
        50007 | 50029 => "listitem",
        50008 => "list",
        50009 => "menu",
        50010 => "menubar",
        50011 => "menuitem",
        50012 => "progressbar",
        50013 => "radiobutton",
        50014 => "scrollbar",
        50015 => "slider",
        50016 => "spinner",
        50017 => "statusbar",
        50018 => "tablist",
        50019 => "tab",
        50020 => "statictext",
        50021 => "toolbar",
        50022 => "tooltip",
        50023 => "tree",
        50024 => "treeitem",
        50025 | 50026 | 50033 => "group",
        50027 => "thumb",
        50028 | 50036 => "table",
        50030 => "document",
        50032 => "window",
        50034 => "header",
        50035 => "headeritem",
        50037 => "titlebar",
        50038 => "separator",
        50039 => "semanticzoom",
        50040 => "appbar",
        _ => "unknown",
    }
}

/// The app name agents use: the program file name without folder or `.exe`.
pub fn app_name_from_path(path: &str) -> Option<String> {
    let file = path.rsplit(['\\', '/']).next()?;
    let stem = file
        .strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".EXE"))
        .unwrap_or(file);
    (!stem.is_empty()).then(|| stem.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_control_types() {
        assert_eq!(canonical_role(50000), "button");
        assert_eq!(canonical_role(50004), "textfield");
        assert_eq!(canonical_role(50032), "window");
        assert_eq!(canonical_role(50026), "group");
        assert_eq!(canonical_role(12345), "unknown");
    }

    #[test]
    fn app_names_drop_folder_and_extension() {
        assert_eq!(
            app_name_from_path(r"C:\Windows\System32\notepad.exe").as_deref(),
            Some("notepad")
        );
        assert_eq!(
            app_name_from_path("C:/Program Files/App/App.EXE").as_deref(),
            Some("App")
        );
        assert_eq!(app_name_from_path(""), None);
    }
}
