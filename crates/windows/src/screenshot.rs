//! Screenshot of the primary display through GDI.

use agentic_desktop_core::image::write_png;
use agentic_desktop_core::{AppError, ErrorCode};
use std::path::Path;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
    CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
    SRCCOPY, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SetProcessDPIAware,
};

fn failed(message: &str) -> AppError {
    AppError::new(ErrorCode::ActionFailed, message)
}

pub fn capture(out: &Path) -> Result<(u32, u32), AppError> {
    unsafe {
        let _ = SetProcessDPIAware();
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        if width <= 0 || height <= 0 {
            return Err(failed("Cannot read the screen size"));
        }
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let previous = SelectObject(memory, bitmap.into());

        let copied = BitBlt(
            memory,
            0,
            0,
            width,
            height,
            Some(screen),
            0,
            0,
            SRCCOPY | CAPTUREBLT,
        );
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let lines = GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );

        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);

        if copied.is_err() || lines == 0 {
            return Err(failed("Cannot copy the screen"));
        }
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        write_png(out, width as u32, height as u32, &pixels)?;
        Ok((width as u32, height as u32))
    }
}
