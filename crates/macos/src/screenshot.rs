//! Screenshot through the `screencapture` tool that ships with macOS.

use agentic_desktop_core::image::png_size;
use agentic_desktop_core::{AppError, ErrorCode};
use std::path::Path;
use std::process::Command;

pub fn capture(out: &Path) -> Result<(u32, u32), AppError> {
    let output = Command::new("screencapture")
        .args(["-x", "-t", "png"])
        .arg(out)
        .output()
        .map_err(|e| {
            AppError::new(
                ErrorCode::ActionFailed,
                format!("Cannot run screencapture: {e}"),
            )
        })?;
    if !output.status.success() || !out.is_file() {
        return Err(AppError::new(
            ErrorCode::PermDenied,
            "The screenshot failed. The terminal may lack the Screen Recording permission",
        )
        .with_suggestion(
            "Open System Settings > Privacy & Security > Screen Recording and allow your terminal",
        ));
    }
    png_size(out)
}
