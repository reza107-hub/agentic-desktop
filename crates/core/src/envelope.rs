use crate::error::{AppError, ErrorCode};
use serde::Serialize;
use serde_json::Value;

pub const ENVELOPE_VERSION: &str = "0.1";

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub requires_fresh_snapshot: bool,
}

/// Every command prints exactly one of these as JSON.
#[derive(Debug, Serialize)]
pub struct Envelope {
    pub version: &'static str,
    pub ok: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Envelope {
    pub fn success(command: &str, data: Value) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            ok: true,
            command: command.to_string(),
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(command: &str, error: &AppError) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            ok: false,
            command: command.to_string(),
            data: None,
            error: Some(ErrorBody {
                code: error.code,
                message: error.message.clone(),
                suggestion: error.suggestion.clone(),
                requires_fresh_snapshot: error.code.needs_fresh_snapshot(),
            }),
        }
    }

    pub fn from_result(command: &str, result: Result<Value, AppError>) -> Self {
        match result {
            Ok(data) => Self::success(command, data),
            Err(error) => Self::failure(command, &error),
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.error.as_ref().map_or(0, |e| e.code.exit_code())
    }
}
