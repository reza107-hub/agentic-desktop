//! Read and act on apps through the macOS Accessibility API.

use crate::ffi::{
    self, AX_ERROR_ACTION_UNSUPPORTED, AX_ERROR_API_DISABLED, AX_ERROR_CANNOT_COMPLETE,
    AX_ERROR_INVALID_ELEMENT, AX_SUCCESS, AXError, Element,
};
use crate::roles::{action_name, app_name_from_path, canonical_role, is_noise_action};
use agentic_desktop_core::{
    AccessibilityNode, Action, ActionReport, AppError, AppInfo, ErrorCode, Rect,
};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const MAX_DEPTH: usize = 40;
const MAX_NODES: usize = 4000;
const TREE_DEADLINE: Duration = Duration::from_secs(15);
const MAX_VALUE_CHARS: usize = 200;
const MESSAGE_TIMEOUT_SECONDS: f32 = 2.0;

/// Actions tried, in order, when a click is asked for.
const CLICK_ACTIONS: &[&str] = &["AXPress", "AXPick", "AXConfirm", "AXOpen"];

fn permission_error() -> AppError {
    AppError::new(
        ErrorCode::PermDenied,
        "Accessibility permission is not granted to this program",
    )
    .with_suggestion(
        "Open System Settings > Privacy & Security > Accessibility and allow your terminal",
    )
}

fn ax_error(code: AXError) -> AppError {
    match code {
        AX_ERROR_API_DISABLED => permission_error(),
        AX_ERROR_INVALID_ELEMENT => AppError::new(ErrorCode::StaleRef, "The element is gone")
            .with_suggestion("Take a new snapshot and use its refs"),
        AX_ERROR_CANNOT_COMPLETE => {
            AppError::new(ErrorCode::Timeout, "The app did not answer in time")
        }
        AX_ERROR_ACTION_UNSUPPORTED => unsupported("do that"),
        other => AppError::new(
            ErrorCode::ActionFailed,
            format!("Accessibility error {other}"),
        ),
    }
}

fn unsupported(what: &str) -> AppError {
    AppError::new(
        ErrorCode::ActionNotSupported,
        format!("This element cannot {what}"),
    )
    .with_suggestion("Take a snapshot and pick an element whose available_actions fit")
}

fn ensure_trusted() -> Result<(), AppError> {
    if unsafe { ffi::AXIsProcessTrusted() } == 0 {
        return Err(permission_error());
    }
    Ok(())
}

/// Running apps as `(name, pid)`, one per app bundle process.
fn running_apps() -> Vec<(String, i32)> {
    ffi::all_pids()
        .into_iter()
        .filter_map(|pid| {
            let name = app_name_from_path(&ffi::process_path(pid)?)?;
            Some((name, pid))
        })
        .collect()
}

pub fn list_apps() -> Result<Vec<AppInfo>, AppError> {
    let mut apps: BTreeMap<String, AppInfo> = BTreeMap::new();
    for (name, pid) in running_apps() {
        apps.entry(name.to_lowercase()).or_insert(AppInfo {
            name,
            pid: u32::try_from(pid).ok(),
        });
    }
    Ok(apps.into_values().collect())
}

fn app_element(app: &str) -> Result<Element, AppError> {
    ensure_trusted()?;
    let matches: Vec<i32> = running_apps()
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(app))
        .map(|(_, pid)| pid)
        .collect();
    let pid = match matches.as_slice() {
        [] => {
            return Err(
                AppError::new(ErrorCode::AppNotFound, format!("No app named '{app}'"))
                    .with_suggestion("Run list-apps to see the available apps"),
            );
        }
        [only] => *only,
        _ => {
            return Err(AppError::new(
                ErrorCode::AmbiguousTarget,
                format!("More than one process is named '{app}'"),
            )
            .with_suggestion("Quit the extra copies and try again"));
        }
    };
    let element =
        Element::application(pid).ok_or_else(|| AppError::internal("Cannot open the app"))?;
    element.set_timeout(MESSAGE_TIMEOUT_SECONDS);
    Ok(element)
}

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

pub fn capture(app: &str) -> Result<AccessibilityNode, AppError> {
    let root = app_element(app)?;
    let mut budget = Budget {
        nodes_left: MAX_NODES,
        deadline: Instant::now() + TREE_DEADLINE,
    };
    let mut node = read_element(&root, 0, &mut budget)
        .ok_or_else(|| AppError::new(ErrorCode::Timeout, "Could not read the app in time"))?;
    if node.role != "application" {
        node.role = "application".to_string();
    }
    node.name = Some(app.to_string());
    Ok(node)
}

fn read_element(element: &Element, depth: usize, budget: &mut Budget) -> Option<AccessibilityNode> {
    if !budget.spend() {
        return None;
    }
    let mut node = read_fields(element);
    let children = element.children().unwrap_or_default();
    if depth >= MAX_DEPTH && !children.is_empty() {
        node.subtree_truncated = true;
        return Some(node);
    }
    for child in &children {
        match read_element(child, depth + 1, budget) {
            Some(child_node) => node.children.push(child_node),
            None => {
                node.subtree_truncated = true;
                break;
            }
        }
    }
    Some(node)
}

fn read_fields(element: &Element) -> AccessibilityNode {
    let ax_role = element.string("AXRole").unwrap_or_default();
    let subrole = element.string("AXSubrole");
    let mut node = AccessibilityNode::new(canonical_role(&ax_role, subrole.as_deref()));
    node.name = element
        .string("AXTitle")
        .or_else(|| element.string("AXDescription"));
    node.native_id = element.string("AXIdentifier");
    if node.role == "textfield" || node.role == "statictext" {
        let value = element
            .string("AXValue")
            .map(|v| v.chars().take(MAX_VALUE_CHARS).collect::<String>());
        if node.role == "statictext" && node.name.is_none() {
            node.name = value;
        } else {
            node.value = value;
        }
    }
    node.bounds = element.bounds().map(|(position, size)| Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    });
    node.available_actions = element
        .action_names()
        .iter()
        .map(|name| action_name(name))
        .filter(|name| !is_noise_action(name))
        .collect();
    node
}

fn element_at(app: &str, path: &[usize]) -> Result<Element, AppError> {
    let mut current = app_element(app)?;
    for &index in path {
        let mut children = current.children().map_err(ax_error)?;
        if index >= children.len() {
            return Err(AppError::new(
                ErrorCode::StaleRef,
                "The element moved while the action was prepared",
            )
            .with_suggestion("Take a new snapshot and use its refs"));
        }
        current = children.swap_remove(index);
    }
    Ok(current)
}

pub fn perform(app: &str, path: &[usize], action: &Action) -> Result<ActionReport, AppError> {
    let element = element_at(app, path)?;
    match action {
        Action::Click => click(&element)?,
        Action::Type(text) => {
            let existing = element.string("AXValue").unwrap_or_default();
            check(element.set_string("AXValue", &format!("{existing}{text}")))?;
        }
        Action::SetValue(value) => check(element.set_string("AXValue", value))?,
    }
    Ok(ActionReport::unverified())
}

fn check(code: AXError) -> Result<(), AppError> {
    if code == AX_SUCCESS {
        Ok(())
    } else {
        Err(ax_error(code))
    }
}

fn click(element: &Element) -> Result<(), AppError> {
    let offered = element.action_names();
    let chosen = CLICK_ACTIONS
        .iter()
        .find(|wanted| offered.iter().any(|name| name == *wanted))
        .ok_or_else(|| unsupported("be clicked"))?;
    check(element.perform(chosen))
}
