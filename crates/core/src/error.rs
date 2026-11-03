use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    PermDenied,
    ElementNotFound,
    AppNotFound,
    ActionFailed,
    ActionNotSupported,
    StaleRef,
    AmbiguousTarget,
    WindowNotFound,
    PlatformNotSupported,
    Timeout,
    InvalidArgs,
    PolicyDenied,
    SnapshotNotFound,
    Internal,
}

impl ErrorCode {
    /// Exit code for the CLI: 2 for bad arguments, 1 for every other error.
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorCode::InvalidArgs => 2,
            _ => 1,
        }
    }

    /// Only these errors are safe to retry after taking a fresh snapshot.
    pub fn needs_fresh_snapshot(self) -> bool {
        matches!(self, ErrorCode::StaleRef | ErrorCode::AmbiguousTarget)
    }
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub suggestion: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn invalid_args(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgs, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn not_supported(what: &str) -> Self {
        Self::new(
            ErrorCode::PlatformNotSupported,
            format!("{what} is not supported on this platform yet"),
        )
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(format!("I/O error: {error}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(format!("JSON error: {error}"))
    }
}
