//! Windows backend, built on UI Automation. Compiled only on Windows.

mod roles;
#[cfg(windows)]
mod uia;

use agentic_desktop_core::PlatformAdapter;
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
