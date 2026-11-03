use crate::error::{AppError, ErrorCode};
use crate::refs::RefMap;
use std::fs;
use std::path::PathBuf;

const MAX_REFMAP_BYTES: u64 = 1_048_576;

/// Where snapshots are kept: `<root>/snapshots/<snapshot_id>/refmap.json`.
pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `AGENTIC_DESKTOP_HOME`, else `~/.agentic-desktop`.
    pub fn from_env() -> Result<Self, AppError> {
        if let Some(path) = std::env::var_os("AGENTIC_DESKTOP_HOME") {
            return Ok(Self::new(PathBuf::from(path)));
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| AppError::internal("Cannot find the home folder"))?;
        Ok(Self::new(PathBuf::from(home).join(".agentic-desktop")))
    }

    fn refmap_path(&self, snapshot_id: &str) -> Result<PathBuf, AppError> {
        if snapshot_id.is_empty() || !snapshot_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(AppError::invalid_args("Invalid snapshot id"));
        }
        Ok(self
            .root
            .join("snapshots")
            .join(snapshot_id)
            .join("refmap.json"))
    }

    pub fn save(&self, refmap: &RefMap) -> Result<(), AppError> {
        let path = self.refmap_path(&refmap.snapshot_id)?;
        let bytes = serde_json::to_vec(refmap)?;
        if bytes.len() as u64 > MAX_REFMAP_BYTES {
            return Err(AppError::internal(
                "Snapshot is too large to store; use a smaller --app scope",
            ));
        }
        let folder = path
            .parent()
            .ok_or_else(|| AppError::internal("Bad snapshot path"))?;
        fs::create_dir_all(folder)?;
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, bytes)?;
        fs::rename(&temp, &path)?;
        Ok(())
    }

    pub fn load(&self, snapshot_id: &str) -> Result<RefMap, AppError> {
        let path = self.refmap_path(snapshot_id)?;
        let metadata = fs::metadata(&path).map_err(|_| not_found(snapshot_id))?;
        if metadata.len() > MAX_REFMAP_BYTES {
            return Err(AppError::internal(
                "Stored snapshot is larger than the allowed size",
            ));
        }
        let bytes = fs::read(&path).map_err(|_| not_found(snapshot_id))?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn not_found(snapshot_id: &str) -> AppError {
    AppError::new(
        ErrorCode::SnapshotNotFound,
        format!("No snapshot '{snapshot_id}'"),
    )
    .with_suggestion("Take a new snapshot")
}
