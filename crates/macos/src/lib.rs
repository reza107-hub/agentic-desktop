//! macOS backend, built on the Accessibility (AX) API. Compiled only on macOS.

#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod clipboard;
#[cfg(target_os = "macos")]
mod ffi;
#[cfg(target_os = "macos")]
mod input;
mod keys;
mod roles;
#[cfg(target_os = "macos")]
mod screenshot;

use agentic_desktop_core::PlatformAdapter;
#[cfg(target_os = "macos")]
use agentic_desktop_core::input::{Combo, MouseButton};
#[cfg(target_os = "macos")]
use agentic_desktop_core::{AccessibilityNode, Action, ActionReport, AppError, AppInfo};

#[derive(Default)]
pub struct MacOsAdapter;

impl MacOsAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_os = "macos"))]
impl PlatformAdapter for MacOsAdapter {
    fn platform(&self) -> &'static str {
        "macos"
    }
}

#[cfg(target_os = "macos")]
impl PlatformAdapter for MacOsAdapter {
    fn platform(&self) -> &'static str {
        "macos"
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AppError> {
        ax::list_apps()
    }

    fn press(&self, combo: &Combo) -> Result<(), AppError> {
        input::press(combo)
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
        ax::capture(app)
    }

    fn perform(
        &self,
        app: &str,
        path: &[usize],
        action: &Action,
    ) -> Result<ActionReport, AppError> {
        ax::perform(app, path, action)
    }
}
