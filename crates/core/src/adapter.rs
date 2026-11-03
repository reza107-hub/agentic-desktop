use crate::error::AppError;
use crate::input::{Combo, MouseButton};
use crate::node::AccessibilityNode;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Click,
    Type(String),
    SetValue(String),
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Action::Click => "click",
            Action::Type(_) => "type",
            Action::SetValue(_) => "set-value",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionReport {
    /// `delivered_verified`, `delivered_unverified` or `not_delivered`.
    pub delivery: String,
}

impl ActionReport {
    pub fn unverified() -> Self {
        Self {
            delivery: "delivered_unverified".to_string(),
        }
    }
}

/// One implementation per operating system. Every method defaults to
/// "not supported", so a backend only writes what it can really do.
pub trait PlatformAdapter {
    fn platform(&self) -> &'static str;

    fn list_apps(&self) -> Result<Vec<AppInfo>, AppError> {
        Err(AppError::not_supported("list-apps"))
    }

    fn capture_tree(&self, _app: &str) -> Result<AccessibilityNode, AppError> {
        Err(AppError::not_supported("snapshot"))
    }

    /// Press a key combination on the real keyboard.
    fn press(&self, _combo: &Combo) -> Result<(), AppError> {
        Err(AppError::not_supported("press"))
    }

    /// Type text with real key presses into whatever has the keyboard focus.
    fn type_text(&self, _text: &str) -> Result<(), AppError> {
        Err(AppError::not_supported("type-text"))
    }

    fn mouse_move(&self, _x: f64, _y: f64) -> Result<(), AppError> {
        Err(AppError::not_supported("mouse-move"))
    }

    fn mouse_click(
        &self,
        _x: f64,
        _y: f64,
        _button: MouseButton,
        _count: u8,
    ) -> Result<(), AppError> {
        Err(AppError::not_supported("mouse-click"))
    }

    /// Scroll by whole wheel steps. Positive `dy` scrolls down, positive `dx` scrolls right.
    fn mouse_wheel(&self, _dx: i32, _dy: i32) -> Result<(), AppError> {
        Err(AppError::not_supported("mouse-wheel"))
    }

    /// Capture the whole screen as a PNG file at `out`, and return its size in pixels.
    fn screenshot(&self, _out: &std::path::Path) -> Result<(u32, u32), AppError> {
        Err(AppError::not_supported("screenshot"))
    }

    fn clipboard_get(&self) -> Result<String, AppError> {
        Err(AppError::not_supported("clipboard-get"))
    }

    fn clipboard_set(&self, _text: &str) -> Result<(), AppError> {
        Err(AppError::not_supported("clipboard-set"))
    }

    fn clipboard_clear(&self) -> Result<(), AppError> {
        self.clipboard_set("")
    }

    /// `path` is the child-index path inside a tree just returned by `capture_tree`.
    fn perform(
        &self,
        _app: &str,
        _path: &[usize],
        _action: &Action,
    ) -> Result<ActionReport, AppError> {
        Err(AppError::not_supported("actions"))
    }
}
