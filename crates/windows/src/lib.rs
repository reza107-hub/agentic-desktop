//! windows backend. Phase 1 stub: every capability reports "not supported" until its phase lands.

use agentic_desktop_core::PlatformAdapter;

#[derive(Default)]
pub struct WindowsAdapter;

impl WindowsAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAdapter for WindowsAdapter {
    fn platform(&self) -> &'static str {
        "windows"
    }
}
