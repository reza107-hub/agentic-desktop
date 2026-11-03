//! Windows backend, built on UI Automation. Compiled only on Windows.

#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod input;
mod keys;
mod roles;
#[cfg(windows)]
mod screenshot;
#[cfg(windows)]
mod uia;

use agentic_desktop_core::PlatformAdapter;
#[cfg(windows)]
use agentic_desktop_core::input::{Combo, MouseButton};
#[cfg(windows)]
use agentic_desktop_core::{AccessibilityNode, Action, ActionReport, AppError, AppInfo};

#[derive(Default)]
pub struct WindowsAdapter;

impl WindowsAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(windows))]
impl PlatformAdapter for WindowsAdapter {
    fn platform(&self) -> &'static str {
        "windows"
    }
}

#[cfg(windows)]
impl PlatformAdapter for WindowsAdapter {
    fn platform(&self) -> &'static str {
        "windows"
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AppError> {
        uia::Uia::new()?.list_apps()
    }

    fn press(&self, combo: &Combo) -> Result<(), AppError> {
        input::press(combo)
    }

    fn type_text(&self, text: &str) -> Result<(), AppError> {
        input::type_text(text)
    }

    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AppError> {
        input::mouse_move(x, y)
    }

    fn mouse_click(&self, x: f64, y: f64, button: MouseButton, count: u8) -> Result<(), AppError> {
        input::mouse_click(x, y, button, count)
    }

    fn mouse_wheel(&self, dx: i32, dy: i32) -> Result<(), AppError> {
        input::mouse_wheel(dx, dy)
    }

    fn screenshot(&self, out: &std::path::Path) -> Result<(u32, u32), AppError> {
        screenshot::capture(out)
    }

    fn clipboard_get(&self) -> Result<String, AppError> {
        clipboard::get()
    }

    fn clipboard_set(&self, text: &str) -> Result<(), AppError> {
        clipboard::set(text)
    }

    fn capture_tree(&self, app: &str) -> Result<AccessibilityNode, AppError> {
        uia::Uia::new()?.capture(app)
    }

    fn perform(
        &self,
        app: &str,
        path: &[usize],
        action: &Action,
    ) -> Result<ActionReport, AppError> {
        uia::Uia::new()?.perform(app, path, action)
    }
}
