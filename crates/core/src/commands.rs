use crate::adapter::{Action, PlatformAdapter};
use crate::error::AppError;
use crate::refs::{parse_ref, resolve};
use crate::snapshot;
use crate::store::SnapshotStore;
use serde_json::{Value, json};

pub fn version() -> Result<Value, AppError> {
    Ok(json!({ "name": "agentic-desktop", "version": env!("CARGO_PKG_VERSION") }))
}

pub fn status(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    Ok(json!({ "platform": adapter.platform(), "version": env!("CARGO_PKG_VERSION") }))
}

pub fn list_apps(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    Ok(json!({ "apps": adapter.list_apps()? }))
}

pub fn take_snapshot(
    adapter: &dyn PlatformAdapter,
    store: &SnapshotStore,
    app: &str,
    interactive_only: bool,
) -> Result<Value, AppError> {
    let result = snapshot::build(adapter, app, interactive_only)?;
    store.save(&result.refmap)?;
    Ok(json!({
        "app": result.app,
        "snapshot_id": result.snapshot_id,
        "ref_count": result.refmap.len(),
        "tree": result.tree,
    }))
}

/// Act on a ref: find the element again in a fresh capture, then perform the action.
pub fn act_on_ref(
    adapter: &dyn PlatformAdapter,
    store: &SnapshotStore,
    ref_text: &str,
    action: Action,
) -> Result<Value, AppError> {
    let (snapshot_id, key) = parse_ref(ref_text)?;
    let refmap = store.load(&snapshot_id)?;
    let entry = refmap.get(&key).ok_or_else(|| {
        AppError::new(
            crate::ErrorCode::ElementNotFound,
            format!("Snapshot has no ref '{ref_text}'"),
        )
        .with_suggestion("Take a new snapshot")
    })?;
    let live_tree = adapter.capture_tree(&entry.app)?;
    let path = resolve(&live_tree, entry)?;
    let report = adapter.perform(&entry.app, &path, &action)?;
    Ok(json!({ "ref": ref_text, "action": action.name(), "result": report }))
}
