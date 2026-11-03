//! C library. One call runs a command line and returns the same JSON line the CLI prints.
//!
//! Rules for callers (also in `include/agentic_desktop.h`):
//! - Pass the arguments after the program name as a JSON array of strings.
//! - Free every returned string with `ad_free_string`, never with `free`.
//! - Calls are independent, but they share the process environment and the state folder.

use agentic_desktop_core::{AppError, Envelope};
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Bumped when a function changes in a way old callers cannot handle.
pub const ABI_VERSION: u32 = 1;

static VERSION: &CStr = c"0.1.0";

#[unsafe(no_mangle)]
pub extern "C" fn ad_abi_version() -> u32 {
    ABI_VERSION
}

/// The library version as a static string. Do not free it.
#[unsafe(no_mangle)]
pub extern "C" fn ad_version() -> *const c_char {
    VERSION.as_ptr()
}

fn envelope_text(envelope: &Envelope) -> String {
    serde_json::to_string(envelope).unwrap_or_else(|_| {
        r#"{"version":"0.1","ok":false,"command":"ffi","error":{"code":"INTERNAL","message":"Cannot encode the result"}}"#
            .to_string()
    })
}

fn failure_text(code_message: AppError) -> String {
    envelope_text(&Envelope::failure("ffi", &code_message))
}

fn into_c_string(text: String) -> *mut c_char {
    CString::new(text.replace('\0', "\u{fffd}"))
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Run `args` (a JSON array of strings, for example `["list-apps"]`) and return the JSON result.
/// Returns null only when memory runs out.
///
/// # Safety
/// `args_json` must be null or point to a NUL-terminated string that stays valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_execute(args_json: *const c_char) -> *mut c_char {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if args_json.is_null() {
            return failure_text(AppError::invalid_args("args_json is null"));
        }
        let text = match unsafe { CStr::from_ptr(args_json) }.to_str() {
            Ok(text) => text,
            Err(_) => return failure_text(AppError::invalid_args("args_json is not valid UTF-8")),
        };
        let args: Vec<String> = match serde_json::from_str(text) {
            Ok(args) => args,
            Err(error) => {
                return failure_text(AppError::invalid_args(format!(
                    "args_json must be a JSON array of strings: {error}"
                )));
            }
        };
        let full = std::iter::once("agentic-desktop".to_string()).chain(args);
        envelope_text(&agentic_desktop_cli::run_to_envelope(full))
    }));
    into_c_string(
        outcome.unwrap_or_else(|_| {
            failure_text(AppError::internal("The library hit an internal error"))
        }),
    )
}

/// Free a string returned by `ad_execute`. Passing null is allowed and does nothing.
///
/// # Safety
/// `text` must be null or a pointer returned by `ad_execute` that has not been freed yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_free_string(text: *mut c_char) {
    if !text.is_null() {
        drop(unsafe { CString::from_raw(text) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(args: &str) -> serde_json::Value {
        let input = CString::new(args).unwrap();
        let output = unsafe { ad_execute(input.as_ptr()) };
        assert!(!output.is_null());
        let text = unsafe { CStr::from_ptr(output) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { ad_free_string(output) };
        serde_json::from_str(&text).expect("the result is JSON")
    }

    #[test]
    fn version_and_abi() {
        assert_eq!(ad_abi_version(), 1);
        let version = unsafe { CStr::from_ptr(ad_version()) }.to_str().unwrap();
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn runs_commands_like_the_cli() {
        let result = call(r#"["version"]"#);
        assert_eq!(result["ok"], true);
        assert_eq!(result["command"], "version");
    }

    #[test]
    fn rejects_bad_input_without_crashing() {
        assert_eq!(call("not json")["error"]["code"], "INVALID_ARGS");
        assert_eq!(call(r#"[1, 2]"#)["error"]["code"], "INVALID_ARGS");
        assert_eq!(
            call(r#"["no-such-command"]"#)["error"]["code"],
            "INVALID_ARGS"
        );
        let output = unsafe { ad_execute(std::ptr::null()) };
        let text = unsafe { CStr::from_ptr(output) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { ad_free_string(output) };
        assert!(text.contains("INVALID_ARGS"));
    }

    #[test]
    fn input_commands_still_need_headed() {
        let result = call(r#"["press","enter"]"#);
        assert_eq!(result["error"]["code"], "POLICY_DENIED");
    }

    #[test]
    fn freeing_null_is_fine() {
        unsafe { ad_free_string(std::ptr::null_mut()) };
    }

    #[test]
    fn header_declares_every_exported_function() {
        let header = include_str!("../include/agentic_desktop.h");
        for name in [
            "ad_abi_version",
            "ad_version",
            "ad_execute",
            "ad_free_string",
        ] {
            assert!(header.contains(name), "{name} is missing from the header");
        }
        assert!(header.contains("AD_ABI_VERSION 1"));
    }
}
