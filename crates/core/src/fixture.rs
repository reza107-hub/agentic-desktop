//! A fake desktop with one app. It lets the CLI and the tests run on any OS
//! without a real accessibility service.

use crate::adapter::{Action, ActionReport, AppInfo, PlatformAdapter};
use crate::error::{AppError, ErrorCode};
use crate::node::AccessibilityNode;
use std::sync::Mutex;

pub const FIXTURE_APP: &str = "Demo";

pub struct FixtureAdapter {
    tree: Mutex<AccessibilityNode>,
    performed: Mutex<Vec<String>>,
}

pub fn demo_tree() -> AccessibilityNode {
    AccessibilityNode::new("window")
        .named("Demo Window")
        .with_children(vec![
            AccessibilityNode::new("toolbar").with_children(vec![
                AccessibilityNode::new("button")
                    .named("Save")
                    .with_native_id("save"),
                AccessibilityNode::new("button")
                    .named("Cancel")
                    .with_native_id("cancel"),
            ]),
            AccessibilityNode::new("textfield")
                .named("Search")
                .with_native_id("search"),
            AccessibilityNode::new("statictext").named("Ready"),
        ])
}

impl FixtureAdapter {
    pub fn new() -> Self {
        Self::with_tree(demo_tree())
    }

    pub fn with_tree(tree: AccessibilityNode) -> Self {
        Self {
            tree: Mutex::new(tree),
            performed: Mutex::new(Vec::new()),
        }
    }

    /// Change what the fake desktop shows, to simulate the UI moving on.
    pub fn set_tree(&self, tree: AccessibilityNode) {
        *self.tree.lock().unwrap() = tree;
    }

    pub fn performed(&self) -> Vec<String> {
        self.performed.lock().unwrap().clone()
    }

    fn check_app(app: &str) -> Result<(), AppError> {
        if app == FIXTURE_APP {
            Ok(())
        } else {
            Err(
                AppError::new(ErrorCode::AppNotFound, format!("No app named '{app}'"))
                    .with_suggestion("Run list-apps to see the available apps"),
            )
        }
    }
}

impl Default for FixtureAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformAdapter for FixtureAdapter {
    fn platform(&self) -> &'static str {
        "fixture"
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AppError> {
        Ok(vec![AppInfo {
            name: FIXTURE_APP.to_string(),
            pid: None,
        }])
    }

    fn capture_tree(&self, app: &str) -> Result<AccessibilityNode, AppError> {
        Self::check_app(app)?;
        Ok(self.tree.lock().unwrap().clone())
    }

    fn perform(
        &self,
        app: &str,
        path: &[usize],
        action: &Action,
    ) -> Result<ActionReport, AppError> {
        Self::check_app(app)?;
        let tree = self.tree.lock().unwrap();
        let node = tree
            .node_at(path)
            .ok_or_else(|| AppError::new(ErrorCode::ElementNotFound, "Path is not in the tree"))?;
        let label = node.name.clone().unwrap_or_else(|| node.role.clone());
        self.performed
            .lock()
            .unwrap()
            .push(format!("{} {}", action.name(), label));
        Ok(ActionReport::unverified())
    }
}
