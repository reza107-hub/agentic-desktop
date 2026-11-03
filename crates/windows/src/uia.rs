//! Read and act on apps through Windows UI Automation.

use crate::roles::{app_name_from_path, canonical_role};
use agentic_desktop_core::{
    AccessibilityNode, Action, ActionReport, AppError, AppInfo, ErrorCode, Rect,
};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
    IUIAutomationInvokePattern, IUIAutomationSelectionItemPattern, IUIAutomationTogglePattern,
    IUIAutomationTreeWalker, IUIAutomationValuePattern, UIA_ExpandCollapsePatternId,
    UIA_InvokePatternId, UIA_PATTERN_ID, UIA_SelectionItemPatternId, UIA_TogglePatternId,
    UIA_ValuePatternId,
};
use windows::core::{BSTR, Interface, PWSTR};

const MAX_DEPTH: usize = 40;
const MAX_NODES: usize = 4000;
const TREE_DEADLINE: Duration = Duration::from_secs(15);
const MAX_VALUE_CHARS: usize = 200;
const ELEMENT_NOT_AVAILABLE: i32 = 0x8004_0201_u32 as i32;
const ACCESS_DENIED: i32 = 0x8007_0005_u32 as i32;

fn win_err(error: windows::core::Error) -> AppError {
    match error.code().0 {
        ELEMENT_NOT_AVAILABLE => {
            AppError::new(ErrorCode::StaleRef, "The element is no longer available")
                .with_suggestion("Take a new snapshot and use its refs")
        }
        ACCESS_DENIED => AppError::new(ErrorCode::PermDenied, "Windows denied access to the app")
            .with_suggestion("Run the terminal at the same integrity level as the app"),
        _ => AppError::internal(format!("UI Automation error: {error}")),
    }
}

fn stale() -> AppError {
    AppError::new(
        ErrorCode::StaleRef,
        "The element moved while the action was prepared",
    )
    .with_suggestion("Take a new snapshot and use its refs")
}

fn unsupported(what: &str) -> AppError {
    AppError::new(
        ErrorCode::ActionNotSupported,
        format!("This element cannot {what}"),
    )
    .with_suggestion("Take a snapshot and pick an element whose available_actions fit")
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

fn pattern<T: Interface>(element: &IUIAutomationElement, id: UIA_PATTERN_ID) -> Option<T> {
    unsafe { element.GetCurrentPattern(id).ok()?.cast::<T>().ok() }
}

pub struct Uia {
    automation: IUIAutomation,
    walker: IUIAutomationTreeWalker,
}

impl Uia {
    pub fn new() -> Result<Self, AppError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).map_err(win_err)?;
            let walker = automation.ControlViewWalker().map_err(win_err)?;
            Ok(Self { automation, walker })
        }
    }

    fn children(&self, element: &IUIAutomationElement) -> Vec<IUIAutomationElement> {
        let mut found = Vec::new();
        let mut next = unsafe { self.walker.GetFirstChildElement(element) }.ok();
        while let Some(child) = next {
            next = unsafe { self.walker.GetNextSiblingElement(&child) }.ok();
            found.push(child);
        }
        found
    }

    fn top_windows(&self) -> Result<Vec<(IUIAutomationElement, String, u32)>, AppError> {
        let root = unsafe { self.automation.GetRootElement() }.map_err(win_err)?;
        let mut windows = Vec::new();
        for window in self.children(&root) {
            let Ok(pid) = (unsafe { window.CurrentProcessId() }) else {
                continue;
            };
            let pid = pid as u32;
            if let Some(name) = process_name(pid) {
                windows.push((window, name, pid));
            }
        }
        Ok(windows)
    }

    pub fn list_apps(&self) -> Result<Vec<AppInfo>, AppError> {
        let mut apps: BTreeMap<String, AppInfo> = BTreeMap::new();
        for (_, name, pid) in self.top_windows()? {
            apps.entry(name.to_lowercase()).or_insert(AppInfo {
                name,
                pid: Some(pid),
            });
        }
        Ok(apps.into_values().collect())
    }

    fn app_windows(&self, app: &str) -> Result<Vec<IUIAutomationElement>, AppError> {
        let windows: Vec<_> = self
            .top_windows()?
            .into_iter()
            .filter(|(_, name, _)| name.eq_ignore_ascii_case(app))
            .map(|(window, _, _)| window)
            .collect();
        if windows.is_empty() {
            return Err(
                AppError::new(ErrorCode::AppNotFound, format!("No app named '{app}'"))
                    .with_suggestion("Run list-apps to see the available apps"),
            );
        }
        Ok(windows)
    }

    /// The app is a synthetic root; each of its windows is a child, in UI Automation order.
    pub fn capture(&self, app: &str) -> Result<AccessibilityNode, AppError> {
        let windows = self.app_windows(app)?;
        let mut budget = Budget {
            nodes_left: MAX_NODES,
            deadline: Instant::now() + TREE_DEADLINE,
        };
        let mut root = AccessibilityNode::new("application").named(app);
        for window in &windows {
            match self.read_element(window, 1, &mut budget) {
                Some(node) => root.children.push(node),
                None => {
                    root.subtree_truncated = true;
                    break;
                }
            }
        }
        Ok(root)
    }

    fn read_element(
        &self,
        element: &IUIAutomationElement,
        depth: usize,
        budget: &mut Budget,
    ) -> Option<AccessibilityNode> {
        if !budget.spend() {
            return None;
        }
        let mut node = read_fields(element);
        let children = self.children(element);
        if depth >= MAX_DEPTH && !children.is_empty() {
            node.subtree_truncated = true;
            return Some(node);
        }
        for child in &children {
            match self.read_element(child, depth + 1, budget) {
                Some(child_node) => node.children.push(child_node),
                None => {
                    node.subtree_truncated = true;
                    break;
                }
            }
        }
        Some(node)
    }

    pub fn perform(
        &self,
        app: &str,
        path: &[usize],
        action: &Action,
    ) -> Result<ActionReport, AppError> {
        let (first, rest) = path
            .split_first()
            .ok_or_else(|| unsupported("be acted on"))?;
        let mut element = self
            .app_windows(app)?
            .into_iter()
            .nth(*first)
            .ok_or_else(stale)?;
        for &index in rest {
            element = self
                .children(&element)
                .into_iter()
                .nth(index)
                .ok_or_else(stale)?;
        }
        match action {
            Action::Click => click(&element)?,
            Action::Type(text) => write_text(&element, text, true)?,
            Action::SetValue(value) => write_text(&element, value, false)?,
        }
        Ok(ActionReport::unverified())
    }
}

fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        app_name_from_path(&String::from_utf16_lossy(&buffer[..size as usize]))
    }
}

fn text_of(value: windows::core::Result<BSTR>) -> Option<String> {
    value.ok().map(|b| b.to_string()).filter(|t| !t.is_empty())
}

fn read_fields(element: &IUIAutomationElement) -> AccessibilityNode {
    let control_type = unsafe { element.CurrentControlType() }
        .map(|c| c.0)
        .unwrap_or(0);
    let mut node = AccessibilityNode::new(canonical_role(control_type));
    node.name = text_of(unsafe { element.CurrentName() });
    node.native_id = text_of(unsafe { element.CurrentAutomationId() });
    node.bounds = unsafe { element.CurrentBoundingRectangle() }
        .ok()
        .filter(|r| r.right > r.left && r.bottom > r.top)
        .map(|r| Rect {
            x: f64::from(r.left),
            y: f64::from(r.top),
            width: f64::from(r.right - r.left),
            height: f64::from(r.bottom - r.top),
        });
    node.available_actions = available_actions(element);
    if node.role == "textfield" || node.role == "document" {
        node.value = pattern::<IUIAutomationValuePattern>(element, UIA_ValuePatternId)
            .and_then(|p| text_of(unsafe { p.CurrentValue() }))
            .map(|v| v.chars().take(MAX_VALUE_CHARS).collect());
    }
    node
}

fn available_actions(element: &IUIAutomationElement) -> Vec<String> {
    let mut actions = Vec::new();
    if pattern::<IUIAutomationInvokePattern>(element, UIA_InvokePatternId).is_some() {
        actions.push("invoke".to_string());
    }
    if pattern::<IUIAutomationTogglePattern>(element, UIA_TogglePatternId).is_some() {
        actions.push("toggle".to_string());
    }
    if pattern::<IUIAutomationSelectionItemPattern>(element, UIA_SelectionItemPatternId).is_some() {
        actions.push("select".to_string());
    }
    if pattern::<IUIAutomationExpandCollapsePattern>(element, UIA_ExpandCollapsePatternId).is_some()
    {
        actions.push("expand".to_string());
    }
    let writable = pattern::<IUIAutomationValuePattern>(element, UIA_ValuePatternId)
        .is_some_and(|p| !unsafe { p.CurrentIsReadOnly() }.is_ok_and(|b| b.as_bool()));
    if writable {
        actions.push("set-value".to_string());
    }
    actions
}

fn click(element: &IUIAutomationElement) -> Result<(), AppError> {
    unsafe {
        if let Some(p) = pattern::<IUIAutomationInvokePattern>(element, UIA_InvokePatternId) {
            return p.Invoke().map_err(win_err);
        }
        if let Some(p) = pattern::<IUIAutomationTogglePattern>(element, UIA_TogglePatternId) {
            return p.Toggle().map_err(win_err);
        }
        if let Some(p) =
            pattern::<IUIAutomationSelectionItemPattern>(element, UIA_SelectionItemPatternId)
        {
            return p.Select().map_err(win_err);
        }
        if let Some(p) =
            pattern::<IUIAutomationExpandCollapsePattern>(element, UIA_ExpandCollapsePatternId)
        {
            return p.Expand().map_err(win_err);
        }
    }
    Err(unsupported("be clicked"))
}

fn write_text(element: &IUIAutomationElement, text: &str, append: bool) -> Result<(), AppError> {
    let pattern = pattern::<IUIAutomationValuePattern>(element, UIA_ValuePatternId)
        .ok_or_else(|| unsupported("take text"))?;
    unsafe {
        if pattern.CurrentIsReadOnly().is_ok_and(|b| b.as_bool()) {
            return Err(unsupported("take text because it is read-only"));
        }
        let mut new_value = String::new();
        if append {
            new_value = pattern
                .CurrentValue()
                .map(|b| b.to_string())
                .unwrap_or_default();
        }
        new_value.push_str(text);
        pattern
            .SetValue(&BSTR::from(new_value.as_str()))
            .map_err(win_err)
    }
}
