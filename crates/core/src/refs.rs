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
    /// False when another actionable element had the same identity at capture time.
    pub unique: bool,
    /// Hash of every ancestor's identity and sibling ordinal. Used only when `unique` is false.
    #[serde(default)]
    pub ancestry: u64,
    /// Position among siblings with the same identity. Used only when `unique` is false.
    #[serde(default)]
    pub ordinal: usize,
}

impl RefEntry {
    fn matches(&self, node: &AccessibilityNode) -> bool {
        node.role == self.role && node.name == self.name && node.native_id == self.native_id
    }
}

/// The live element a ref points at.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub path: Vec<usize>,
    /// True when the element was found by its position, because looks-alike elements exist.
    pub positional: bool,
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

const FNV_PRIME: u64 = 0x100000001b3;

fn mix(mut hash: u64, text: &str) -> u64 {
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    (hash ^ 0xff).wrapping_mul(FNV_PRIME)
}

fn mix_option(hash: u64, text: &Option<String>) -> u64 {
    match text {
        Some(text) => mix(mix(hash, "S"), text),
        None => mix(hash, "N"),
    }
}

/// The ancestry hash a node passes down to its children.
fn child_ancestry(ancestry: u64, node: &AccessibilityNode, ordinal: usize) -> u64 {
    let hash = mix(ancestry, &node.role);
    let hash = mix_option(hash, &node.name);
    let hash = mix_option(hash, &node.native_id);
    mix(hash, &ordinal.to_string())
}

fn walk<'a>(
    node: &'a AccessibilityNode,
    path: &mut Vec<usize>,
    ancestry: u64,
    ordinal: usize,
    visit: &mut impl FnMut(&'a AccessibilityNode, &[usize], u64, usize),
) {
    visit(node, path, ancestry, ordinal);
    let below = child_ancestry(ancestry, node, ordinal);
    let mut seen: HashMap<Identity, usize> = HashMap::new();
    for (index, child) in node.children.iter().enumerate() {
        let slot = seen.entry(identity(child)).or_insert(0);
        let child_ordinal = *slot;
        *slot += 1;
        path.push(index);
        walk(child, path, below, child_ordinal, visit);
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
        let mut sites: Vec<(Vec<usize>, Identity, u64, usize)> = Vec::new();
        walk(
            tree,
            &mut Vec::new(),
            0,
            0,
            &mut |node, path, ancestry, ordinal| {
                if node.is_actionable() {
                    sites.push((path.to_vec(), identity(node), ancestry, ordinal));
                }
            },
        );

        let mut counts: HashMap<Identity, usize> = HashMap::new();
        for (_, ident, _, _) in &sites {
            *counts.entry(ident.clone()).or_insert(0) += 1;
        }

        let mut entries = BTreeMap::new();
        for (index, (path, ident, ancestry, ordinal)) in sites.into_iter().enumerate() {
            let key = format!("e{}", index + 1);
            if let Some(node) = tree.node_at_mut(&path) {
                node.ref_id = Some(format!("@{snapshot_id}:{key}"));
            }
            let unique = counts.get(&ident).copied().unwrap_or(0) == 1;
            let (role, name, native_id) = ident;
            entries.insert(
                key,
                RefEntry {
                    path,
                    role,
                    name,
                    native_id,
                    app: app.to_string(),
                    unique,
                    ancestry,
                    ordinal,
                },
            );
        }

        RefMap {
            snapshot_id: snapshot_id.to_string(),
            app: app.to_string(),
            entries,
        }
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

fn fresh_snapshot_error(code: ErrorCode, message: &str) -> AppError {
    AppError::new(code, message).with_suggestion("Take a new snapshot and use its refs")
}

/// Find the one live node a ref points at, or explain why not.
///
/// A unique element is found by identity alone: zero matches is STALE_REF,
/// two or more is AMBIGUOUS_TARGET. An element that had looks-alikes at capture
/// time is found by identity plus its place in the tree, and the result says so.
pub fn resolve(live_tree: &AccessibilityNode, entry: &RefEntry) -> Result<Resolved, AppError> {
    let mut found: Vec<(Vec<usize>, u64, usize)> = Vec::new();
    walk(
        live_tree,
        &mut Vec::new(),
        0,
        0,
        &mut |node, path, ancestry, ordinal| {
            if entry.matches(node) {
                found.push((path.to_vec(), ancestry, ordinal));
            }
        },
    );

    if entry.unique {
        return match found.len() {
            0 => Err(fresh_snapshot_error(
                ErrorCode::StaleRef,
                "The element is no longer on screen",
            )),
            1 => Ok(Resolved {
                path: found.remove(0).0,
                positional: false,
            }),
            _ => Err(fresh_snapshot_error(
                ErrorCode::AmbiguousTarget,
                "More than one element now matches this ref",
            )),
        };
    }

    found.retain(|(_, ancestry, ordinal)| *ancestry == entry.ancestry && *ordinal == entry.ordinal);
    match found.len() {
        0 => Err(fresh_snapshot_error(
            ErrorCode::StaleRef,
            "No element with this look is at the same position any more",
        )),
        1 => Ok(Resolved {
            path: found.remove(0).0,
            positional: true,
        }),
        _ => Err(fresh_snapshot_error(
            ErrorCode::AmbiguousTarget,
            "More than one element matches this position",
        )),
    }
}
