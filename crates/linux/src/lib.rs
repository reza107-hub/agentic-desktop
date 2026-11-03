//! Linux backend, built on AT-SPI over D-Bus.

mod actions;
mod bus;
mod roles;
mod tree;

use agentic_desktop_core::{
    AccessibilityNode, Action, ActionReport, AppError, AppInfo, PlatformAdapter,
};
use bus::{ACCESSIBLE, Atspi, Target};

#[derive(Default)]
pub struct LinuxAdapter;

impl LinuxAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAdapter for LinuxAdapter {
    fn platform(&self) -> &'static str {
        "linux"
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AppError> {
        let atspi = Atspi::connect()?;
        let mut apps = Vec::new();
        for app in atspi.children(&Target::registry_root())? {
            let Ok(name) = atspi.string_property(&app, ACCESSIBLE, "Name") else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            apps.push(AppInfo {
                name,
                pid: atspi.process_id(&app.bus),
            });
        }
        apps.sort_by_key(|app| app.name.to_lowercase());
        Ok(apps)
    }

    fn capture_tree(&self, app: &str) -> Result<AccessibilityNode, AppError> {
        let atspi = Atspi::connect()?;
        let target = atspi.find_app(app)?;
        atspi.capture_app(&target)
    }

    fn perform(
        &self,
        app: &str,
        path: &[usize],
        action: &Action,
    ) -> Result<ActionReport, AppError> {
        let atspi = Atspi::connect()?;
        let root = atspi.find_app(app)?;
        let target = atspi.target_at(&root, path)?;
        atspi.perform_on(&target, action)
    }
}
