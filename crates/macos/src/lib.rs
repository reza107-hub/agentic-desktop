//! macOS backend, built on the Accessibility (AX) API. Compiled only on macOS.

#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod ffi;
mod roles;

use agentic_desktop_core::PlatformAdapter;
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
