//! Linux backend, built on AT-SPI over D-Bus.

mod actions;
mod bus;
mod clipboard;
mod input;
mod keys;
mod roles;
mod screenshot;
mod tree;

use agentic_desktop_core::input::{Combo, MouseButton};
use agentic_desktop_core::{
    AccessibilityNode, Action, ActionReport, AppError, AppInfo, PlatformAdapter,
};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A number that is different on every call in this process.
pub(crate) fn unique_counter() -> u32 {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
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

    fn clipboard_clear(&self) -> Result<(), AppError> {
        clipboard::clear()
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

#[cfg(test)]
mod live_tests {
    use super::bus::Atspi;
    use agentic_desktop_core::AccessibilityNode;

    type Signature = (
        Vec<usize>,
        String,
        Option<String>,
        Option<String>,
        Vec<String>,
    );

    /// The cache names one role number once per app, so "tabpanel" and "group" can swap.
    fn same_role(role: &str) -> String {
        if role == "tabpanel" { "group" } else { role }.to_string()
    }

    fn actionable(node: &AccessibilityNode, path: &mut Vec<usize>, out: &mut Vec<Signature>) {
        if node.is_actionable() {
            let mut actions = node.available_actions.clone();
            actions.sort();
            out.push((
                path.clone(),
                same_role(&node.role),
                node.name.clone(),
                node.native_id.clone(),
                actions,
            ));
        }
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            actionable(child, path, out);
            path.pop();
        }
    }

    /// The fast cache reader must find the same actionable elements as the slow walk.
    /// Run with: cargo test -p agentic-desktop-linux -- --ignored --nocapture cache_matches_slow_walk
    #[test]
    #[ignore = "needs a live desktop session with Nautilus running"]
    fn cache_matches_slow_walk() {
        let atspi = Atspi::connect().unwrap();
        let Ok(app) = atspi.find_app("org.gnome.Nautilus") else {
            println!("SKIPPED: Nautilus is not running");
            return;
        };
        let started = std::time::Instant::now();
        let fast = atspi.capture_app(&app).unwrap();
        let fast_time = started.elapsed();
        let started = std::time::Instant::now();
        let slow = atspi.capture_slow(&app).unwrap();
        let slow_time = started.elapsed();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        actionable(&fast, &mut Vec::new(), &mut a);
        actionable(&slow, &mut Vec::new(), &mut b);
        println!(
            "fast {fast_time:?} ({} actionable), slow {slow_time:?} ({} actionable)",
            a.len(),
            b.len()
        );
        assert_eq!(a, b);
    }
}
