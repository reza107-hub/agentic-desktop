use crate::adapter::{Action, PlatformAdapter};
use crate::error::AppError;
use crate::input::{MouseButton, parse_combo};
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

pub fn press(adapter: &dyn PlatformAdapter, combo_text: &str) -> Result<Value, AppError> {
    let combo = parse_combo(combo_text)?;
    adapter.press(&combo)?;
    Ok(json!({ "pressed": combo_text.trim().to_lowercase() }))
}

fn check_point(x: f64, y: f64) -> Result<(), AppError> {
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 || x > 100_000.0 || y > 100_000.0 {
        return Err(AppError::invalid_args(
            "Mouse position must be a screen position from 0 to 100000",
        ));
    }
    Ok(())
}

pub fn mouse_move(adapter: &dyn PlatformAdapter, x: f64, y: f64) -> Result<Value, AppError> {
    check_point(x, y)?;
    adapter.mouse_move(x, y)?;
    Ok(json!({ "x": x, "y": y }))
}

pub fn mouse_click(
    adapter: &dyn PlatformAdapter,
    x: f64,
    y: f64,
    button: &str,
    count: u8,
) -> Result<Value, AppError> {
    check_point(x, y)?;
    if !(1..=3).contains(&count) {
        return Err(AppError::invalid_args("Click count must be 1, 2 or 3"));
    }
    let button = MouseButton::parse(button)?;
    adapter.mouse_click(x, y, button, count)?;
    Ok(json!({ "x": x, "y": y, "button": format!("{button:?}").to_lowercase(), "count": count }))
}

pub fn mouse_wheel(adapter: &dyn PlatformAdapter, dx: i32, dy: i32) -> Result<Value, AppError> {
    if dx == 0 && dy == 0 {
        return Err(AppError::invalid_args("Give a non-zero --dx or --dy"));
    }
    if dx.unsigned_abs() > 1000 || dy.unsigned_abs() > 1000 {
        return Err(AppError::invalid_args(
            "Scroll by at most 1000 steps at a time",
        ));
    }
    adapter.mouse_wheel(dx, dy)?;
    Ok(json!({ "dx": dx, "dy": dy }))
}

/// Take a screenshot into `out`, or into the state folder when no path is given.
/// An existing file is never overwritten.
pub fn screenshot(
    adapter: &dyn PlatformAdapter,
    store: &SnapshotStore,
    out: Option<&str>,
) -> Result<Value, AppError> {
    let path = match out {
        Some(text) => std::path::PathBuf::from(text),
        None => store
            .root()
            .join("screenshots")
            .join(format!("{}.png", snapshot::new_snapshot_id())),
    };
    if path.exists() {
        return Err(
            AppError::invalid_args(format!("'{}' already exists", path.display()))
                .with_suggestion("Pick a new file name"),
        );
    }
    if let Some(folder) = path.parent().filter(|f| !f.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder)?;
    }
    let (width, height) = adapter.screenshot(&path)?;
    Ok(json!({ "path": path.display().to_string(), "width": width, "height": height }))
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
