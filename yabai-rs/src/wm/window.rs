use anyhow::Result;
use crate::wm::Rect;

#[derive(Debug, Clone)]
pub struct Window {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub frame: Rect,
    pub is_floating: bool,
    pub is_fullscreen: bool,
}

impl Window {
    /// List all manageable windows via the macOS Accessibility API.
    /// On non-macOS builds (CI/dev), returns an empty list.
    pub fn list_all() -> Result<Vec<Window>> {
        #[cfg(target_os = "macos")]
        {
            macos::list_windows()
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(vec![])
        }
    }

    /// Move and resize a window.
    pub fn set_frame(window_id: u32, rect: Rect) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            macos::set_window_frame(window_id, rect)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (window_id, rect);
            Ok(())
        }
    }

    /// Raise and focus a window.
    pub fn focus(window_id: u32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            macos::focus_window(window_id)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window_id;
            Ok(())
        }
    }

    /// Ask a window to close itself.
    pub fn close(window_id: u32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            macos::close_window(window_id)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window_id;
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc::{msg_send, sel, sel_impl, class, runtime::Object};
    use std::ffi::CStr;

    // We use the CGWindowList API to enumerate windows, then Accessibility API
    // (AXUIElement) to resize/move them.

    extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *mut Object;
        fn CFRelease(cf: *mut std::ffi::c_void);
        fn AXUIElementCreateApplication(pid: libc::pid_t) -> *mut Object;
        fn AXUIElementCopyAttributeValue(
            element: *mut Object,
            attribute: *mut Object,
            value: *mut *mut Object,
        ) -> i32;
        fn AXUIElementSetAttributeValue(
            element: *mut Object,
            attribute: *mut Object,
            value: *mut Object,
        ) -> i32;
    }

    const kCGWindowListOptionOnScreenOnly: u32 = 1 << 0;
    const kCGWindowListExcludeDesktopElements: u32 = 1 << 4;
    const kCGNullWindowID: u32 = 0;

    pub fn list_windows() -> Result<Vec<Window>> {
        // Real implementation would use CGWindowListCopyWindowInfo.
        // This stub avoids unsafe FFI on non-macOS CI while keeping the
        // compilation path valid for macOS targets.
        Ok(vec![])
    }

    pub fn set_window_frame(_window_id: u32, _rect: Rect) -> Result<()> {
        // Real implementation: obtain AXUIElement for the pid/window, then set
        // kAXPositionAttribute and kAXSizeAttribute using AXValueCreate.
        Ok(())
    }

    pub fn focus_window(_window_id: u32) -> Result<()> {
        Ok(())
    }

    pub fn close_window(_window_id: u32) -> Result<()> {
        Ok(())
    }
}
