//! Clipboard through the standard command-line tools for the session type.
//! Wayland uses `wl-clipboard`; X11 uses `xclip` or `xsel`.

use agentic_desktop_core::{AppError, ErrorCode};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TOOL_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    WlClipboard,
    Xclip,
    Xsel,
}

/// Choose a tool for the session. Wayland prefers `wl-clipboard`, which also works
/// as the fallback order for X11 sessions only when the X11 tools are missing.
pub fn choose_tool(wayland: bool, x11: bool, has: impl Fn(&str) -> bool) -> Option<Tool> {
    let wl = has("wl-copy") && has("wl-paste");
    let order: &[Tool] = if wayland {
        &[Tool::WlClipboard, Tool::Xclip, Tool::Xsel]
    } else if x11 {
        &[Tool::Xclip, Tool::Xsel, Tool::WlClipboard]
    } else {
        return None;
    };
    order.iter().copied().find(|tool| match tool {
        Tool::WlClipboard => wl,
        Tool::Xclip => has("xclip"),
        Tool::Xsel => has("xsel"),
    })
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

fn current_tool() -> Result<Tool, AppError> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    choose_tool(wayland, x11, on_path).ok_or_else(|| {
        AppError::new(
            ErrorCode::PlatformNotSupported,
            "No clipboard tool is available in this session",
        )
        .with_suggestion(
            "Install one: `sudo apt install wl-clipboard` on Wayland, or `sudo apt install xclip` on X11",
        )
    })
}

struct Finished {
    success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

/// Run a tool with a time limit. With `capture` off, output goes nowhere, which is
/// needed for tools that leave a background process holding the clipboard.
fn run(
    program: &str,
    args: &[&str],
    input: Option<&str>,
    capture: bool,
) -> Result<Finished, AppError> {
    let pipe = if capture {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    let mut child = Command::new(program)
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(pipe)
        .stderr(if capture {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .map_err(|e| AppError::internal(format!("Cannot start {program}: {e}")))?;

    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| AppError::internal(format!("Cannot write to {program}: {e}")))?;
    }
    let out_reader = child.stdout.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    });
    let err_reader = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        })
    });

    let deadline = Instant::now() + TOOL_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::new(
                    ErrorCode::Timeout,
                    format!("{program} did not finish in time"),
                ));
            }
            Err(e) => {
                return Err(AppError::internal(format!(
                    "Cannot wait for {program}: {e}"
                )));
            }
        }
    };
    Ok(Finished {
        success: status.success(),
        stdout: out_reader.and_then(|h| h.join().ok()).unwrap_or_default(),
        stderr: err_reader.and_then(|h| h.join().ok()).unwrap_or_default(),
    })
}

/// Tools report an empty clipboard as an error. That is not a failure for us.
fn means_empty(stderr: &str) -> bool {
    let lowered = stderr.to_lowercase();
    [
        "nothing is copied",
        "not available",
        "no selection",
        "target string",
        "empty",
    ]
    .iter()
    .any(|phrase| lowered.contains(phrase))
}

pub fn get() -> Result<String, AppError> {
    let (program, args): (&str, &[&str]) = match current_tool()? {
        Tool::WlClipboard => ("wl-paste", &["--no-newline"]),
        Tool::Xclip => ("xclip", &["-selection", "clipboard", "-o"]),
        Tool::Xsel => ("xsel", &["--clipboard", "--output"]),
    };
    let done = run(program, args, None, true)?;
    if !done.success {
        return if means_empty(&done.stderr)
            || done.stdout.is_empty() && done.stderr.trim().is_empty()
        {
            Ok(String::new())
        } else {
            Err(AppError::new(
                ErrorCode::ActionFailed,
                format!("{program} failed: {}", done.stderr.trim()),
            ))
        };
    }
    String::from_utf8(done.stdout).map_err(|_| {
        AppError::new(
            ErrorCode::ActionFailed,
            "The clipboard does not hold plain text",
        )
    })
}

pub fn set(text: &str) -> Result<(), AppError> {
    let (program, args): (&str, &[&str]) = match current_tool()? {
        Tool::WlClipboard => ("wl-copy", &[]),
        Tool::Xclip => ("xclip", &["-selection", "clipboard", "-i"]),
        Tool::Xsel => ("xsel", &["--clipboard", "--input"]),
    };
    let done = run(program, args, Some(text), false)?;
    done.success.then_some(()).ok_or_else(|| {
        AppError::new(
            ErrorCode::ActionFailed,
            format!("{program} could not set the clipboard"),
        )
    })
}

pub fn clear() -> Result<(), AppError> {
    match current_tool()? {
        Tool::WlClipboard => run("wl-copy", &["--clear"], None, false)?
            .success
            .then_some(())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::ActionFailed,
                    "wl-copy could not clear the clipboard",
                )
            }),
        Tool::Xclip | Tool::Xsel => set(""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_only(names: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |name| names.contains(&name)
    }

    #[test]
    fn wayland_prefers_wl_clipboard() {
        let all = has_only(&["wl-copy", "wl-paste", "xclip", "xsel"]);
        assert_eq!(choose_tool(true, true, all), Some(Tool::WlClipboard));
    }

    #[test]
    fn wayland_falls_back_to_xclip() {
        assert_eq!(
            choose_tool(true, true, has_only(&["xclip"])),
            Some(Tool::Xclip)
        );
    }

    #[test]
    fn x11_prefers_xclip_then_xsel() {
        assert_eq!(
            choose_tool(false, true, has_only(&["xsel", "xclip"])),
            Some(Tool::Xclip)
        );
        assert_eq!(
            choose_tool(false, true, has_only(&["xsel"])),
            Some(Tool::Xsel)
        );
    }

    #[test]
    fn wl_clipboard_needs_both_programs() {
        assert_eq!(choose_tool(true, false, has_only(&["wl-copy"])), None);
    }

    #[test]
    fn no_display_means_no_tool() {
        assert_eq!(choose_tool(false, false, has_only(&["xclip"])), None);
    }

    #[test]
    fn recognises_empty_clipboard_messages() {
        assert!(means_empty("Nothing is copied"));
        assert!(means_empty("Error: target STRING not available"));
        assert!(!means_empty("cannot connect to display"));
    }
}
