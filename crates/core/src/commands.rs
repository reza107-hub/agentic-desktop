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

/// Largest clipboard text returned or accepted, in bytes.
pub const MAX_CLIPBOARD_BYTES: usize = 1_048_576;

pub fn clipboard_get(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let text = adapter.clipboard_get()?;
    if text.len() > MAX_CLIPBOARD_BYTES {
        return Err(AppError::new(
            crate::ErrorCode::ActionFailed,
            format!("The clipboard holds more than {MAX_CLIPBOARD_BYTES} bytes of text"),
        ));
    }
    Ok(json!({ "text": text, "chars": text.chars().count() }))
}

pub fn clipboard_set(adapter: &dyn PlatformAdapter, text: &str) -> Result<Value, AppError> {
    if text.len() > MAX_CLIPBOARD_BYTES {
        return Err(AppError::invalid_args(format!(
            "Clipboard text is larger than {MAX_CLIPBOARD_BYTES} bytes"
        )));
    }
    adapter.clipboard_set(text)?;
    Ok(json!({ "chars": text.chars().count() }))
}

pub fn clipboard_clear(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    adapter.clipboard_clear()?;
    Ok(json!({ "cleared": true }))
}

pub fn take_snapshot(
    adapter: &dyn PlatformAdapter,
    store: &SnapshotStore,
    app: &str,
    interactive_only: bool,
) -> Result<Value, AppError> {
    let result = snapshot::build(adapter, app, interactive_only)?;
    store.save(&result.refmap)?;
    let mut data = json!({
        "app": result.app,
        "snapshot_id": result.snapshot_id,
        "ref_count": result.refmap.len(),
        "complete": result.complete,
        "tree": result.tree,
    });
    if !result.complete {
        data["truncated"] = json!(true);
    }
    Ok(data)
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
    let resolved = resolve(&live_tree, entry)?;
    let report = adapter.perform(&entry.app, &resolved.path, &action)?;
    let matched_by = if resolved.positional {
        "position"
    } else {
        "identity"
    };
    Ok(json!({
        "ref": ref_text,
        "action": action.name(),
        "matched_by": matched_by,
        "result": report,
    }))
}
