//! Clipboard text through the Win32 clipboard API (UTF-16 text format).

use agentic_desktop_core::{AppError, ErrorCode};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;

const OPEN_TIMEOUT: Duration = Duration::from_secs(2);

/// Another program may hold the clipboard for a moment, so retry briefly.
struct Open;

impl Open {
    fn acquire() -> Result<Open, AppError> {
        let deadline = Instant::now() + OPEN_TIMEOUT;
        loop {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Open);
            }
            if Instant::now() > deadline {
                return Err(AppError::new(
                    ErrorCode::Timeout,
                    "Another program is holding the clipboard",
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

fn failed(message: &str) -> AppError {
    AppError::new(ErrorCode::ActionFailed, message)
}

pub fn get() -> Result<String, AppError> {
    let _open = Open::acquire()?;
    unsafe {
        let Ok(handle) = GetClipboardData(CF_UNICODETEXT.0.into()) else {
            return Ok(String::new());
        };
        let global = HGLOBAL(handle.0);
        let bytes = GlobalSize(global);
        let pointer = GlobalLock(global) as *const u16;
        if pointer.is_null() {
            return Err(failed("Cannot read the clipboard"));
        }
        let units = std::slice::from_raw_parts(pointer, bytes / 2);
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        let text = String::from_utf16_lossy(&units[..end]);
        let _ = GlobalUnlock(global);
        Ok(text)
    }
}

pub fn set(text: &str) -> Result<(), AppError> {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    let _open = Open::acquire()?;
    unsafe {
        EmptyClipboard().map_err(|_| failed("Cannot empty the clipboard"))?;
        if text.is_empty() {
            return Ok(());
        }
        let size = units.len() * 2;
        let global = GlobalAlloc(GMEM_MOVEABLE, size).map_err(|_| failed("Out of memory"))?;
        let pointer = GlobalLock(global) as *mut u16;
        if pointer.is_null() {
            return Err(failed("Cannot prepare the clipboard text"));
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len());
        let _ = GlobalUnlock(global);
        SetClipboardData(CF_UNICODETEXT.0.into(), Some(HANDLE(global.0)))
            .map_err(|_| failed("Cannot set the clipboard"))?;
    }
    Ok(())
}
