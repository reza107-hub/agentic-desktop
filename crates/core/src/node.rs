use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Roles that always get a ref, because an agent can act on them.
pub const INTERACTIVE_ROLES: &[&str] = &[
    "button",
    "link",
    "textfield",
    "checkbox",
    "radiobutton",
    "menuitem",
    "tab",
    "combobox",
    "slider",
    "switch",
    "treeitem",
    "listitem",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AccessibilityNode {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<AccessibilityNode>,
}

impl AccessibilityNode {
    pub fn new(role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            ..Self::default()
        }
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_native_id(mut self, id: impl Into<String>) -> Self {
        self.native_id = Some(id.into());
        self
    }

    pub fn with_actions(mut self, actions: &[&str]) -> Self {
        self.available_actions = actions.iter().map(|a| a.to_string()).collect();
        self
    }

    pub fn with_children(mut self, children: Vec<AccessibilityNode>) -> Self {
        self.children = children;
        self
    }

    /// A node earns a ref when its role is interactive or it offers a primary action.
    pub fn is_actionable(&self) -> bool {
        INTERACTIVE_ROLES.contains(&self.role.as_str()) || !self.available_actions.is_empty()
    }

    /// Follow child indices from this node. Returns None if the path leaves the tree.
    pub fn node_at(&self, path: &[usize]) -> Option<&AccessibilityNode> {
        let mut current = self;
        for &index in path {
            current = current.children.get(index)?;
        }
        Some(current)
    }
}
