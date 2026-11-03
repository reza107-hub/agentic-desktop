//! Read an app's accessibility tree from AT-SPI.

use crate::bus::{ACCESSIBLE, ACTION, Atspi, COMPONENT, CacheItem, TEXT, Target};
use crate::roles::{STATE_EDITABLE, canonical_role, has_state};
use agentic_desktop_core::node::INTERACTIVE_ROLES;
use agentic_desktop_core::{AccessibilityNode, AppError, ErrorCode, Rect};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use zbus::zvariant::OwnedValue;

const MAX_DEPTH: usize = 40;
const MAX_NODES: usize = 4000;
const TREE_DEADLINE: Duration = Duration::from_secs(15);
const MAX_VALUE_CHARS: i32 = 200;
/// Most extra threads reading at once. Beyond this, elements are read on the calling thread.
const MAX_THREADS: usize = 16;

enum Slot<'scope> {
    Running(std::thread::ScopedJoinHandle<'scope, Option<AccessibilityNode>>),
    Done(Box<Option<AccessibilityNode>>),
}

struct Budget {
    nodes_left: AtomicUsize,
    threads_running: AtomicUsize,
    deadline: Instant,
}

impl Budget {
    fn try_start_thread(&self) -> bool {
        if self.threads_running.fetch_add(1, Ordering::SeqCst) < MAX_THREADS {
            return true;
        }
        self.threads_running.fetch_sub(1, Ordering::SeqCst);
        false
    }

    fn finish_thread(&self) {
        self.threads_running.fetch_sub(1, Ordering::SeqCst);
    }

    fn spend(&self) -> bool {
        if Instant::now() > self.deadline {
            return false;
        }
        loop {
            let left = self.nodes_left.load(Ordering::SeqCst);
            if left == 0 {
                return false;
            }
            let swapped = self.nodes_left.compare_exchange(
                left,
                left - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            if swapped.is_ok() {
                return true;
            }
        }
    }
}

fn text_of(properties: &HashMap<String, OwnedValue>, name: &str) -> Option<String> {
    let value = properties.get(name)?.try_clone().ok()?;
    String::try_from(value).ok().filter(|text| !text.is_empty())
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

    /// Read an app. Descriptive fields come from the one-call cache when the app offers one.
    pub fn capture_app(&self, app: &Target) -> Result<AccessibilityNode, AppError> {
        let cache = self.load_cache(app);
        self.capture_with(app, cache.as_ref())
    }

    /// Walk the app one element at a time with no cache. Used to check the cached read.
    #[cfg(test)]
    pub fn capture_slow(&self, app: &Target) -> Result<AccessibilityNode, AppError> {
        self.capture_with(app, None)
    }

    fn capture_with(
        &self,
        app: &Target,
        cache: Option<&Cache>,
    ) -> Result<AccessibilityNode, AppError> {
        let budget = Budget {
            nodes_left: AtomicUsize::new(MAX_NODES),
            threads_running: AtomicUsize::new(0),
            deadline: Instant::now() + TREE_DEADLINE,
        };
        self.read_node(app, 0, &budget, cache)
            .ok_or_else(|| AppError::new(ErrorCode::Timeout, "Could not read the app in time"))
    }

    /// Children are never filtered, so a child index here is the index AT-SPI uses.
    fn read_node(
        &self,
        target: &Target,
        depth: usize,
        budget: &Budget,
        cache: Option<&Cache>,
    ) -> Option<AccessibilityNode> {
        if !budget.spend() {
            return None;
        }
        let mut node = self
            .read_fields(target, cache)
            .unwrap_or_else(|_| AccessibilityNode::new("unavailable"));
        let Ok(children) = self.children(target) else {
            return Some(node);
        };
        if children.is_empty() {
            return Some(node);
        }
        if depth >= MAX_DEPTH {
            node.subtree_truncated = true;
            return Some(node);
        }
        for child in self.read_children(&children, depth, budget, cache) {
            match child {
                Some(child_node) => node.children.push(child_node),
                None => {
                    node.subtree_truncated = true;
                    break;
                }
            }
        }
        Some(node)
    }

    fn read_children(
        &self,
        children: &[Target],
        depth: usize,
        budget: &Budget,
        cache: Option<&Cache>,
    ) -> Vec<Option<AccessibilityNode>> {
        if children.len() < 2 {
            return children
                .iter()
                .map(|child| self.read_node(child, depth + 1, budget, cache))
                .collect();
        }
        std::thread::scope(|scope| {
            let slots: Vec<Slot> = children
                .iter()
                .map(|child| {
                    if budget.try_start_thread() {
                        Slot::Running(scope.spawn(move || {
                            let node = self.read_node(child, depth + 1, budget, cache);
                            budget.finish_thread();
                            node
                        }))
                    } else {
                        Slot::Done(Box::new(self.read_node(child, depth + 1, budget, cache)))
                    }
                })
                .collect();
            slots
                .into_iter()
                .map(|slot| match slot {
                    Slot::Running(handle) => handle.join().unwrap_or(None),
                    Slot::Done(node) => *node,
                })
                .collect()
        })
    }

    fn read_fields(
        &self,
        target: &Target,
        cache: Option<&Cache>,
    ) -> Result<AccessibilityNode, AppError> {
        if let Some((mut node, item)) = cache.and_then(|c| c.node_for(&target.path)) {
            if wants_extras(&node.role, &item.interfaces) {
                self.read_extras(target, &node.role, &item.interfaces)
                    .apply(&mut node);
            }
            return Ok(node);
        }
        let role_name: String = self.call(target, ACCESSIBLE, "GetRoleName", &())?;
        let states: Vec<u32> = self
            .call(target, ACCESSIBLE, "GetState", &())
            .unwrap_or_default();
        let interfaces: Vec<String> = self
            .call(target, ACCESSIBLE, "GetInterfaces", &())
            .unwrap_or_default();
        let properties = self.all_properties(target, ACCESSIBLE).unwrap_or_default();

        let mut node = AccessibilityNode::new(canonical_role(
            &role_name,
            has_state(&states, STATE_EDITABLE),
        ));
        node.name = text_of(&properties, "Name");
        node.description = text_of(&properties, "Description");
        if wants_extras(&node.role, &interfaces) {
            self.read_extras(target, &node.role, &interfaces)
                .apply(&mut node);
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
        Some(text).filter(|t| !t.is_empty())
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

fn has_interface(interfaces: &[String], suffix: &str) -> bool {
    interfaces.iter().any(|i| i.ends_with(suffix))
}

/// Only elements an agent can act on, and windows, need the slower per-element reads.
fn wants_extras(role: &str, interfaces: &[String]) -> bool {
    INTERACTIVE_ROLES.contains(&role) || role == "window" || has_interface(interfaces, ".Action")
}

/// Details that the cache does not carry, read per element.
#[derive(Default)]
struct Extras {
    native_id: Option<String>,
    actions: Vec<String>,
    value: Option<String>,
    bounds: Option<Rect>,
}

impl Extras {
    fn apply(self, node: &mut AccessibilityNode) {
        node.native_id = self.native_id;
        node.available_actions = self.actions;
        node.value = self.value;
        node.bounds = self.bounds;
    }
}

impl Atspi {
    fn read_extras(&self, target: &Target, role: &str, interfaces: &[String]) -> Extras {
        let native_id = self
            .string_property(target, ACCESSIBLE, "AccessibleId")
            .ok()
            .filter(|id| !id.is_empty());
        let actions = if has_interface(interfaces, ".Action") {
            self.action_names(target)
        } else {
            Vec::new()
        };
        let value = (role == "textfield" && has_interface(interfaces, ".Text"))
            .then(|| self.read_text(target))
            .flatten();
        let bounds = has_interface(interfaces, ".Component")
            .then(|| self.read_bounds(target))
            .flatten();
        Extras {
            native_id,
            actions,
            value,
            bounds,
        }
    }

    /// Resolve each distinct role number once. Elements that share a role number
    /// in one app share a role name, which is how toolkits report them.
    fn role_names(&self, items: &[CacheItem]) -> HashMap<u32, String> {
        let mut names = HashMap::new();
        for item in items {
            if names.contains_key(&item.role) {
                continue;
            }
            if let Ok(name) = self.call::<_, String>(&item.target, ACCESSIBLE, "GetRoleName", &()) {
                names.insert(item.role, name);
            }
        }
        names
    }
}

/// Descriptive fields for every element of one app, fetched with a single call.
/// The cache is only trusted for these fields: GTK4 reports wrong child counts and
/// indices in it, so the tree shape always comes from the app itself.
struct Cache {
    by_path: HashMap<String, CacheItem>,
    role_names: HashMap<u32, String>,
}

impl Cache {
    fn node_for(&self, path: &str) -> Option<(AccessibilityNode, &CacheItem)> {
        let item = self.by_path.get(path)?;
        let role_name = self.role_names.get(&item.role)?;
        let editable = has_state(&item.states, STATE_EDITABLE);
        let mut node = AccessibilityNode::new(canonical_role(role_name, editable));
        node.name = Some(item.name.clone()).filter(|name| !name.is_empty());
        node.description = Some(item.description.clone()).filter(|text| !text.is_empty());
        Some((node, item))
    }
}

impl Atspi {
    fn load_cache(&self, app: &Target) -> Option<Cache> {
        let items = self.cache_items(&app.bus).ok()?;
        let items: Vec<CacheItem> = items
            .into_iter()
            .filter(|i| i.target.bus == app.bus)
            .collect();
        let role_names = self.role_names(&items);
        let by_path = items
            .into_iter()
            .map(|item| (item.target.path.clone(), item))
            .collect();
        Some(Cache {
            by_path,
            role_names,
        })
    }
}
