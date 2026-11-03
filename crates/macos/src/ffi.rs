//! The few C functions we need from CoreFoundation, ApplicationServices and libproc,
//! plus an owning wrapper so every CoreFoundation object is released exactly once.

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

/// Release a CoreFoundation object we created and no longer need.
pub unsafe fn release(object: CFTypeRef) {
    unsafe { CFRelease(object) }
}

pub type CFTypeRef = *const c_void;
pub type CFIndex = isize;
pub type AXError = i32;

pub const AX_SUCCESS: AXError = 0;
pub const AX_ERROR_API_DISABLED: AXError = -25211;
pub const AX_ERROR_INVALID_ELEMENT: AXError = -25202;
pub const AX_ERROR_CANNOT_COMPLETE: AXError = -25204;
pub const AX_ERROR_ACTION_UNSUPPORTED: AXError = -25206;

const UTF8: u32 = 0x0800_0100;
const AX_VALUE_CGPOINT: u32 = 1;
const AX_VALUE_CGSIZE: u32 = 2;
const PROC_ALL_PIDS: u32 = 1;
const PROC_PIDPATH_MAX: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        num_bytes: CFIndex,
        encoding: u32,
        is_external: u8,
    ) -> CFTypeRef;
    fn CFStringGetLength(string: CFTypeRef) -> CFIndex;
    fn CFStringGetMaximumSizeForEncoding(length: CFIndex, encoding: u32) -> CFIndex;
    fn CFStringGetCString(
        string: CFTypeRef,
        buffer: *mut c_char,
        size: CFIndex,
        encoding: u32,
    ) -> u8;
    fn CFArrayGetCount(array: CFTypeRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: CFIndex) -> CFTypeRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateApplication(pid: c_int) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyActionNames(element: CFTypeRef, names: *mut CFTypeRef) -> AXError;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFTypeRef) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, seconds: f32) -> AXError;
    fn AXValueGetValue(value: CFTypeRef, kind: u32, out: *mut c_void) -> u8;
}

unsafe extern "C" {
    fn proc_listpids(kind: u32, info: u32, buffer: *mut c_void, size: c_int) -> c_int;
    fn proc_pidpath(pid: c_int, buffer: *mut c_void, size: u32) -> c_int;
}

/// An owned CoreFoundation object. Dropping it releases it.
pub struct Cf(CFTypeRef);

impl Drop for Cf {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

impl Cf {
    fn owned(pointer: CFTypeRef) -> Option<Cf> {
        (!pointer.is_null()).then(|| Cf(pointer))
    }

    pub fn text(value: &str) -> Cf {
        let pointer = unsafe {
            CFStringCreateWithBytes(
                std::ptr::null(),
                value.as_ptr(),
                value.len() as CFIndex,
                UTF8,
                0,
            )
        };
        Cf(pointer)
    }

    fn is(&self, type_id: usize) -> bool {
        unsafe { CFGetTypeID(self.0) == type_id }
    }

    pub fn as_string(&self) -> Option<String> {
        if !self.is(unsafe { CFStringGetTypeID() }) {
            return None;
        }
        unsafe {
            let length = CFStringGetLength(self.0);
            let capacity = CFStringGetMaximumSizeForEncoding(length, UTF8) + 1;
            let mut buffer = vec![0u8; capacity as usize];
            if CFStringGetCString(self.0, buffer.as_mut_ptr().cast(), capacity, UTF8) == 0 {
                return None;
            }
            let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
            String::from_utf8(buffer[..end].to_vec()).ok()
        }
    }

    /// Items of an array. Each item is retained by the array, so we copy the strings out.
    pub fn strings(&self) -> Vec<String> {
        if !self.is(unsafe { CFArrayGetTypeID() }) {
            return Vec::new();
        }
        let count = unsafe { CFArrayGetCount(self.0) };
        (0..count)
            .filter_map(|index| {
                let item = unsafe { CFArrayGetValueAtIndex(self.0, index) };
                retained(item).and_then(|cf| cf.as_string())
            })
            .collect()
    }

    pub fn elements(&self) -> Vec<Element> {
        if !self.is(unsafe { CFArrayGetTypeID() }) {
            return Vec::new();
        }
        let count = unsafe { CFArrayGetCount(self.0) };
        (0..count)
            .filter_map(|index| {
                let item = unsafe { CFArrayGetValueAtIndex(self.0, index) };
                retained(item).map(Element)
            })
            .collect()
    }

    fn point(&self) -> Option<CGPoint> {
        let mut out = CGPoint::default();
        let ok =
            unsafe { AXValueGetValue(self.0, AX_VALUE_CGPOINT, (&mut out as *mut CGPoint).cast()) };
        (ok != 0).then_some(out)
    }

    fn size(&self) -> Option<CGSize> {
        let mut out = CGSize::default();
        let ok =
            unsafe { AXValueGetValue(self.0, AX_VALUE_CGSIZE, (&mut out as *mut CGSize).cast()) };
        (ok != 0).then_some(out)
    }
}

/// Wrap an object we do not own by retaining it first.
fn retained(pointer: CFTypeRef) -> Option<Cf> {
    if pointer.is_null() {
        return None;
    }
    Cf::owned(unsafe { CFRetain(pointer) })
}

/// One accessibility element.
pub struct Element(Cf);

impl Element {
    pub fn application(pid: i32) -> Option<Element> {
        Cf::owned(unsafe { AXUIElementCreateApplication(pid) }).map(Element)
    }

    pub fn set_timeout(&self, seconds: f32) {
        unsafe { AXUIElementSetMessagingTimeout((self.0).0, seconds) };
    }

    pub fn attribute(&self, name: &str) -> Result<Option<Cf>, AXError> {
        let key = Cf::text(name);
        let mut value: CFTypeRef = std::ptr::null();
        let code = unsafe { AXUIElementCopyAttributeValue((self.0).0, key.0, &mut value) };
        match code {
            AX_SUCCESS => Ok(Cf::owned(value)),
            other => Err(other),
        }
    }

    pub fn string(&self, name: &str) -> Option<String> {
        self.attribute(name)
            .ok()
            .flatten()
            .and_then(|v| v.as_string())
            .filter(|text| !text.is_empty())
    }

    pub fn children(&self) -> Result<Vec<Element>, AXError> {
        Ok(self
            .attribute("AXChildren")?
            .map(|array| array.elements())
            .unwrap_or_default())
    }

    pub fn bounds(&self) -> Option<(CGPoint, CGSize)> {
        let position = self.attribute("AXPosition").ok().flatten()?.point()?;
        let size = self.attribute("AXSize").ok().flatten()?.size()?;
        (size.width > 0.0 && size.height > 0.0).then_some((position, size))
    }

    pub fn action_names(&self) -> Vec<String> {
        let mut names: CFTypeRef = std::ptr::null();
        let code = unsafe { AXUIElementCopyActionNames((self.0).0, &mut names) };
        if code != AX_SUCCESS {
            return Vec::new();
        }
        Cf::owned(names)
            .map(|array| array.strings())
            .unwrap_or_default()
    }

    pub fn perform(&self, action: &str) -> AXError {
        let name = Cf::text(action);
        unsafe { AXUIElementPerformAction((self.0).0, name.0) }
    }

    pub fn set_string(&self, attribute: &str, value: &str) -> AXError {
        let key = Cf::text(attribute);
        let text = Cf::text(value);
        unsafe { AXUIElementSetAttributeValue((self.0).0, key.0, text.0) }
    }
}

/// Every process id on the machine.
pub fn all_pids() -> Vec<i32> {
    unsafe {
        let bytes = proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0);
        if bytes <= 0 {
            return Vec::new();
        }
        let mut pids = vec![0i32; bytes as usize / std::mem::size_of::<i32>() + 64];
        let written = proc_listpids(
            PROC_ALL_PIDS,
            0,
            pids.as_mut_ptr().cast(),
            (pids.len() * std::mem::size_of::<i32>()) as c_int,
        );
        if written <= 0 {
            return Vec::new();
        }
        pids.truncate(written as usize / std::mem::size_of::<i32>());
        pids.retain(|&pid| pid > 0);
        pids
    }
}

pub fn process_path(pid: i32) -> Option<String> {
    let mut buffer = vec![0u8; PROC_PIDPATH_MAX];
    let length = unsafe { proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    (length > 0).then(|| String::from_utf8_lossy(&buffer[..length as usize]).into_owned())
}
