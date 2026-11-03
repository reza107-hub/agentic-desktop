//! Read an app's accessibility tree from AT-SPI.

use crate::bus::{ACCESSIBLE, ACTION, Atspi, COMPONENT, TEXT, Target};
use crate::roles::{STATE_EDITABLE, canonical_role, has_state};
use agentic_desktop_core::{AccessibilityNode, AppError, ErrorCode, Rect};
use std::time::{Duration, Instant};

const MAX_DEPTH: usize = 40;
const MAX_NODES: usize = 4000;
const TREE_DEADLINE: Duration = Duration::from_secs(15);
const MAX_VALUE_CHARS: i32 = 200;

struct Budget {
    nodes_left: usize,
    deadline: Instant,
}

impl Budget {
    fn spend(&mut self) -> bool {
        if self.nodes_left == 0 || Instant::now() > self.deadline {
            return false;
        }
        self.nodes_left -= 1;
        true
    }
}

impl Atspi {
    /// Find the one app on the bus whose name matches, ignoring letter case.
    pub fn find_app(&self, name: &str) -> Result<Target, AppError> {
        let mut found = Vec::new();
        for app in self.children(&Target::registry_root())? {
            if let Ok(app_name) = self.string_property(&app, ACCESSIBLE, "Name")
                && app_name.eq_ignore_ascii_case(name)
            {
                found.push(app);
            }
        }
        match found.len() {
            0 => Err(
                AppError::new(ErrorCode::AppNotFound, format!("No app named '{name}'"))
                    .with_suggestion("Run list-apps to see the available apps"),
            ),
            1 => Ok(found.remove(0)),
            _ => Err(AppError::new(
                ErrorCode::AmbiguousTarget,
                format!("More than one app is named '{name}'"),
            )
            .with_suggestion("Close the extra copies and try again")),
        }
    }

    pub fn capture_app(&self, app: &Target) -> Result<AccessibilityNode, AppError> {
        let mut budget = Budget {
            nodes_left: MAX_NODES,
            deadline: Instant::now() + TREE_DEADLINE,
        };
        let root = self.read_node(app, 0, &mut budget);
        root.ok_or_else(|| AppError::new(ErrorCode::Timeout, "Could not read the app in time"))
    }

    /// Children are never filtered, so a child index here is the index AT-SPI uses.
    fn read_node(
        &self,
        target: &Target,
        depth: usize,
        budget: &mut Budget,
    ) -> Option<AccessibilityNode> {
        if !budget.spend() {
            return None;
        }
        let mut node = self
            .read_fields(target)
            .unwrap_or_else(|_| AccessibilityNode::new("unavailable"));
        if depth < MAX_DEPTH
            && let Ok(children) = self.children(target)
        {
            for child in children {
                match self.read_node(&child, depth + 1, budget) {
                    Some(child_node) => node.children.push(child_node),
                    None => break,
                }
            }
        }
        Some(node)
    }

    fn read_fields(&self, target: &Target) -> Result<AccessibilityNode, AppError> {
        let role_name: String = self.call(target, ACCESSIBLE, "GetRoleName", &())?;
        let states: Vec<u32> = self
            .call(target, ACCESSIBLE, "GetState", &())
            .unwrap_or_default();
        let interfaces: Vec<String> = self
            .call(target, ACCESSIBLE, "GetInterfaces", &())
            .unwrap_or_default();
        let has = |suffix: &str| interfaces.iter().any(|i| i.ends_with(suffix));

        let mut node = AccessibilityNode::new(canonical_role(
            &role_name,
            has_state(&states, STATE_EDITABLE),
        ));
        node.name = non_empty(self.string_property(target, ACCESSIBLE, "Name").ok());
        node.description = non_empty(self.string_property(target, ACCESSIBLE, "Description").ok());
        node.native_id = non_empty(
            self.string_property(target, ACCESSIBLE, "AccessibleId")
                .ok(),
        );

        if has(".Action") {
            node.available_actions = self.action_names(target);
        }
        if node.role == "textfield" && has(".Text") {
            node.value = self.read_text(target);
        }
        if (node.is_actionable() || node.role == "window") && has(".Component") {
            node.bounds = self.read_bounds(target);
        }
        Ok(node)
    }

    pub fn action_names(&self, target: &Target) -> Vec<String> {
        let count = self.int_property(target, ACTION, "NActions").unwrap_or(0);
        (0..count)
            .filter_map(|index| {
                self.call::<_, String>(target, ACTION, "GetName", &(index,))
                    .ok()
            })
            .map(|name| name.to_lowercase())
            .collect()
    }

    fn read_text(&self, target: &Target) -> Option<String> {
        let count = self.int_property(target, TEXT, "CharacterCount").ok()?;
        let text: String = self
            .call(target, TEXT, "GetText", &(0i32, count.min(MAX_VALUE_CHARS)))
            .ok()?;
        non_empty(Some(text))
    }

    fn read_bounds(&self, target: &Target) -> Option<Rect> {
        let (x, y, width, height): (i32, i32, i32, i32) =
            self.call(target, COMPONENT, "GetExtents", &(0u32,)).ok()?;
        (width > 0 && height > 0).then_some(Rect {
            x: x.into(),
            y: y.into(),
            width: width.into(),
            height: height.into(),
        })
    }

    /// Follow a child-index path down from an app, using the same indices `capture_app` used.
    pub fn target_at(&self, app: &Target, path: &[usize]) -> Result<Target, AppError> {
        let mut current = app.clone();
        for &index in path {
            let children = self.children(&current)?;
            current = children.into_iter().nth(index).ok_or_else(|| {
                AppError::new(
                    ErrorCode::StaleRef,
                    "The element moved while the action was prepared",
                )
                .with_suggestion("Take a new snapshot and use its refs")
            })?;
        }
        Ok(current)
    }
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|t| !t.is_empty())
}
