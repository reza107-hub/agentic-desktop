//! Platform-neutral core: types, refs, the adapter trait and the JSON envelope.
//! This crate never depends on a platform crate.

pub mod adapter;
pub mod commands;
pub mod envelope;
pub mod error;
pub mod fixture;
pub mod node;
pub mod refs;
pub mod snapshot;
pub mod store;

pub use adapter::{Action, ActionReport, AppInfo, PlatformAdapter};
pub use envelope::Envelope;
pub use error::{AppError, ErrorCode};
pub use node::{AccessibilityNode, Rect};
pub use refs::{RefEntry, RefMap};
pub use snapshot::SnapshotResult;
pub use store::SnapshotStore;
