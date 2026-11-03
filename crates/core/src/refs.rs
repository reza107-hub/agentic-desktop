use crate::error::{AppError, ErrorCode};
use crate::node::AccessibilityNode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// What a ref remembers about its element, so it can be found again later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefEntry {
    pub path: Vec<usize>,
    pub role: String,
    pub name: Option<String>,
    pub native_id: Option<String>,
    pub app: String,
    /// False when another element had the same identity at capture time.
    pub unique: bool,
}

impl RefEntry {
    fn matches(&self, node: &AccessibilityNode) -> bool {
        node.role == self.role && node.name == self.name && node.native_id == self.native_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefMap {
    pub snapshot_id: String,
    pub app: String,
    entries: BTreeMap<String, RefEntry>,
}

type Identity = (String, Option<String>, Option<String>);

fn identity(node: &AccessibilityNode) -> Identity {
    (node.role.clone(), node.name.clone(), node.native_id.clone())
}

fn walk<'a>(
    node: &'a AccessibilityNode,
    path: &mut Vec<usize>,
    visit: &mut impl FnMut(&'a AccessibilityNode, &[usize]),
) {
    visit(node, path);
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        walk(child, path, visit);
        path.pop();
    }
}

impl RefMap {
    pub fn get(&self, key: &str) -> Option<&RefEntry> {
        self.entries.get(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Give every actionable node a ref, depth first, and write the qualified
    /// ref (`@<snapshot>:e<n>`) onto the node.
    pub fn allocate(tree: &mut AccessibilityNode, snapshot_id: &str, app: &str) -> RefMap {
        let mut counts: HashMap<Identity, usize> = HashMap::new();
        walk(tree, &mut Vec::new(), &mut |node, _| {
            if node.is_actionable() {
                *counts.entry(identity(node)).or_insert(0) += 1;
            }
        });

        let mut entries = BTreeMap::new();
        let mut counter = 0u32;
        allocate_into(tree, &mut Vec::new(), &mut |node, path| {
            counter += 1;
            let key = format!("e{counter}");
            node.ref_id = Some(format!("@{snapshot_id}:{key}"));
            entries.insert(
                key,
                RefEntry {
                    path: path.to_vec(),
                    role: node.role.clone(),
                    name: node.name.clone(),
                    native_id: node.native_id.clone(),
                    app: app.to_string(),
                    unique: counts.get(&identity(node)).copied().unwrap_or(0) == 1,
                },
            );
        });

        RefMap {
            snapshot_id: snapshot_id.to_string(),
            app: app.to_string(),
            entries,
        }
    }
}

fn allocate_into(
    node: &mut AccessibilityNode,
    path: &mut Vec<usize>,
    assign: &mut impl FnMut(&mut AccessibilityNode, &[usize]),
) {
    if node.is_actionable() {
        assign(node, path);
    }
    for index in 0..node.children.len() {
        path.push(index);
        allocate_into(&mut node.children[index], path, assign);
        path.pop();
    }
}

/// Split `@<snapshot>:e<n>` into its two parts. A bare `@e8` is rejected.
pub fn parse_ref(text: &str) -> Result<(String, String), AppError> {
    let bad = || {
        AppError::invalid_args(format!("'{text}' is not a valid ref"))
            .with_suggestion("Use the full ref from a snapshot, like @s8f3k2p9:e8")
    };
    let body = text.strip_prefix('@').ok_or_else(bad)?;
    let (snapshot_id, key) = body.split_once(':').ok_or_else(bad)?;
    let snapshot_ok =
        !snapshot_id.is_empty() && snapshot_id.chars().all(|c| c.is_ascii_alphanumeric());
    let key_ok = key
        .strip_prefix('e')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    if !snapshot_ok || !key_ok {
        return Err(bad());
    }
    Ok((snapshot_id.to_string(), key.to_string()))
}

/// Find the one live node a ref points at, or explain why not.
/// Zero matches is STALE_REF; two or more is AMBIGUOUS_TARGET.
pub fn resolve(live_tree: &AccessibilityNode, entry: &RefEntry) -> Result<Vec<usize>, AppError> {
    if !entry.unique {
        return Err(AppError::new(
            ErrorCode::AmbiguousTarget,
            "This element looked the same as another one when the snapshot was taken",
        )
        .with_suggestion("Take a new snapshot and pick a more specific element"));
    }
    let mut found: Vec<Vec<usize>> = Vec::new();
    walk(live_tree, &mut Vec::new(), &mut |node, path| {
        if entry.matches(node) {
            found.push(path.to_vec());
        }
    });
    match found.len() {
        0 => Err(
            AppError::new(ErrorCode::StaleRef, "The element is no longer on screen")
                .with_suggestion("Take a new snapshot and use its refs"),
        ),
        1 => Ok(found.remove(0)),
        _ => Err(AppError::new(
            ErrorCode::AmbiguousTarget,
            "More than one element matches this ref",
        )
        .with_suggestion("Take a new snapshot and pick a more specific element")),
    }
}
