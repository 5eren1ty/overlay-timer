//! Windows-specific border handling for the fullscreen overlay.
//!
//! `winit` refreshes the non-client frame when mouse pass-through changes.
//! Reapplying the invisible DWM border after those refreshes prevents the
//! one-pixel top border from becoming visible again.

#[cfg(target_os = "windows")]
mod platform {
    use std::{ffi::c_void, mem::size_of};

    use windows::{
        Win32::{
            Foundation::HWND,
            Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DwmSetWindowAttribute},
            UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId},
        },
        core::{PCWSTR, w},
    };

    fn find_overlay() -> Option<HWND> {
        let mut previous = None;
        loop {
            // Let Windows filter by title instead of synchronously querying
            // every winit helper window, which could block during startup.
            let Ok(hwnd) =
                (unsafe { FindWindowExW(None, previous, PCWSTR::null(), w!("Overlay Timer")) })
            else {
                return None;
            };

            let mut process_id = 0;
            // SAFETY: `hwnd` was returned by FindWindowExW and the output
            // pointer remains valid for this call.
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
            if process_id == std::process::id() {
                return Some(hwnd);
            }
            previous = Some(hwnd);
        }
    }

    pub(super) fn ensure_overlay_window_configured() {
        let Some(hwnd) = find_overlay() else {
            return;
        };

        let border_color = DWMWA_COLOR_NONE;
        // SAFETY: `hwnd` belongs to this process and `border_color` is passed
        // with the exact size expected for DWMWA_BORDER_COLOR. Reapplying this
        // attribute is idempotent and intentionally follows winit frame changes.
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                (&raw const border_color).cast::<c_void>(),
                size_of::<u32>() as u32,
            )
        };
    }
}

/// Reapplies the native border suppression to this process' overlay window.
///
/// The overlay may not exist yet and winit may refresh its frame later, so
/// callers should invoke this once per root UI frame.
pub fn ensure_overlay_window_configured() {
    #[cfg(target_os = "windows")]
    platform::ensure_overlay_window_configured();
}
