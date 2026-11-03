//! macos backend. Phase 1 stub: every capability reports "not supported" until its phase lands.

use agentic_desktop_core::PlatformAdapter;

#[derive(Default)]
pub struct MacOsAdapter;

impl MacOsAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAdapter for MacOsAdapter {
    fn platform(&self) -> &'static str {
        "macos"
    }
}
