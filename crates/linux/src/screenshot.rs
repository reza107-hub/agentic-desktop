//! Screenshots. Wayland asks the desktop portal; X11 reads the screen directly.

use agentic_desktop_core::image::{png_size, write_png};
use agentic_desktop_core::{AppError, ErrorCode};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};
use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{MatchRule, message::Type};

/// The portal may show a permission dialog the first time, so wait a while.
const PORTAL_TIMEOUT: Duration = Duration::from_secs(60);

fn failed(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ActionFailed, message)
}

pub fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland")
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

pub fn capture(out: &Path) -> Result<(u32, u32), AppError> {
    if is_wayland() {
        portal_capture(out)
    } else {
        x11_capture(out)
    }
}

/// Turn `file:///home/me/Pictures/A%20b.png` into a path.
pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = rest.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

/// The object path where the portal will answer a request with this token.
pub fn request_path(unique_name: &str, token: &str) -> String {
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
}

fn portal_capture(out: &Path) -> Result<(u32, u32), AppError> {
    let connection =
        Connection::session().map_err(|e| failed(format!("Cannot reach the session bus: {e}")))?;
    let unique = connection
        .unique_name()
        .map(|name| name.to_string())
        .ok_or_else(|| failed("The session bus gave no name"))?;
    let token = format!("agentic{}x{}", std::process::id(), crate::unique_counter());
    let path = request_path(&unique, &token);

    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface("org.freedesktop.portal.Request")
        .and_then(|b| b.member("Response"))
        .and_then(|b| b.path(path.as_str()))
        .map_err(|e| failed(format!("Bad match rule: {e}")))?
        .build();
    let mut answers = MessageIterator::for_match_rule(rule, &connection, None)
        .map_err(|e| failed(format!("Cannot listen for the portal answer: {e}")))?;

    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        if let Some(Ok(message)) = answers.next() {
            let body = message
                .body()
                .deserialize::<(u32, HashMap<String, OwnedValue>)>();
            let _ = sender.send(body.map_err(|e| e.to_string()));
        }
    });

    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("interactive", Value::from(false));
    connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Screenshot"),
            "Screenshot",
            &("", options),
        )
        .map_err(|e| failed(format!("The screenshot portal refused the request: {e}")))?;

    let (code, results) = receiver
        .recv_timeout(PORTAL_TIMEOUT)
        .map_err(|_| {
            AppError::new(
                ErrorCode::Timeout,
                "The screenshot portal did not answer in time",
            )
        })?
        .map_err(|e| failed(format!("Bad portal answer: {e}")))?;
    if code != 0 {
        return Err(AppError::new(
            ErrorCode::PermDenied,
            "The screenshot was cancelled or not allowed",
        )
        .with_suggestion("Allow screenshots for this program in the desktop's permission prompt"));
    }
    let uri = results
        .get("uri")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| String::try_from(v).ok())
        .ok_or_else(|| failed("The portal gave no file"))?;
    let source =
        path_from_uri(&uri).ok_or_else(|| failed("The portal gave an unreadable file address"))?;
    std::fs::copy(&source, out)?;
    if source != out {
        let _ = std::fs::remove_file(&source);
    }
    png_size(out)
}

fn x11_capture(out: &Path) -> Result<(u32, u32), AppError> {
    let (connection, screen_number) =
        x11rb::connect(None).map_err(|e| failed(format!("Cannot open the X11 display: {e}")))?;
    let screen = &connection.setup().roots[screen_number];
    let (width, height) = (screen.width_in_pixels, screen.height_in_pixels);
    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            screen.root,
            0,
            0,
            width,
            height,
            u32::MAX,
        )
        .map_err(|e| failed(format!("Cannot read the screen: {e}")))?
        .reply()
        .map_err(|e| failed(format!("Cannot read the screen: {e}")))?;
    let pixels = usize::from(width) * usize::from(height);
    if image.data.len() != pixels * 4 || !(image.depth == 24 || image.depth == 32) {
        return Err(failed(
            "The screen uses a colour format this tool cannot read",
        ));
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for pixel in image.data.as_chunks::<4>().0 {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }
    write_png(out, u32::from(width), u32::from(height), &rgba)?;
    Ok((u32::from(width), u32::from(height)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            path_from_uri("file:///home/me/Pictures/Screen%20shot.png"),
            Some(PathBuf::from("/home/me/Pictures/Screen shot.png"))
        );
        assert_eq!(path_from_uri("https://example.com/a.png"), None);
        assert_eq!(path_from_uri("file:///bad%zz"), None);
    }

    #[test]
    fn builds_the_portal_request_path() {
        assert_eq!(
            request_path(":1.234", "tok"),
            "/org/freedesktop/portal/desktop/request/1_234/tok"
        );
    }
}
