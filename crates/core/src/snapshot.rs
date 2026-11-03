use crate::adapter::PlatformAdapter;
use crate::error::AppError;
use crate::node::AccessibilityNode;
use crate::refs::RefMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct SnapshotResult {
    pub snapshot_id: String,
    pub app: String,
    pub tree: AccessibilityNode,
    pub refmap: RefMap,
    /// False when the backend could not read the whole window.
    pub complete: bool,
}

/// A short id like `s8f3k2p9`: time, process id and a counter, hashed.
pub fn new_snapshot_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hash: u64 = 0xcbf29ce484222325;
    for part in [
        nanos as u64,
        (nanos >> 64) as u64,
        std::process::id() as u64,
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ] {
        for byte in part.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut id = String::from("s");
    for _ in 0..8 {
        id.push(DIGITS[(hash % 36) as usize] as char);
        hash /= 36;
    }
    id
}

/// Keep only nodes that carry a ref, or lead to one. The root always stays.
fn prune_to_interactive(node: &mut AccessibilityNode) -> bool {
    node.children.retain_mut(prune_to_interactive);
    node.ref_id.is_some() || !node.children.is_empty()
}

pub fn build(
    adapter: &dyn PlatformAdapter,
    app: &str,
    interactive_only: bool,
) -> Result<SnapshotResult, AppError> {
    let mut tree = adapter.capture_tree(app)?;
    let snapshot_id = new_snapshot_id();
    let refmap = RefMap::allocate(&mut tree, &snapshot_id, app);
    let complete = !tree.any_truncated();
    if interactive_only {
        prune_to_interactive(&mut tree);
    }
    Ok(SnapshotResult {
        snapshot_id,
        app: app.to_string(),
        tree,
        refmap,
        complete,
    })
}
