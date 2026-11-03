//! Clipboard through `pbpaste` and `pbcopy`, which ship with macOS.

use agentic_desktop_core::{AppError, ErrorCode};
use std::io::Write;
use std::process::{Command, Stdio};

fn failed(message: String) -> AppError {
    AppError::new(ErrorCode::ActionFailed, message)
}

pub fn get() -> Result<String, AppError> {
    let output = Command::new("pbpaste")
        .env("LANG", "en_US.UTF-8")
        .output()
        .map_err(|e| failed(format!("Cannot run pbpaste: {e}")))?;
    if !output.status.success() {
        return Err(failed("pbpaste failed".to_string()));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| failed("The clipboard does not hold plain text".to_string()))
}

pub fn set(text: &str) -> Result<(), AppError> {
    let mut child = Command::new("pbcopy")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| failed(format!("Cannot run pbcopy: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| failed(format!("Cannot write to pbcopy: {e}")))?;
    }
    let status = child
        .wait()
        .map_err(|e| failed(format!("Cannot wait for pbcopy: {e}")))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| failed("pbcopy failed".to_string()))
}
