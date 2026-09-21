//! Physical inset geometry for the normal, shadowless overlay window.
//! Visibility is controlled through egui only after preparation has succeeded.
use std::{ffi::c_void, mem::size_of};

use windows::{
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::{
            Dwm::{DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DwmSetWindowAttribute},
            Gdi::ClientToScreen,
        },
        UI::WindowsAndMessaging::{
            FindWindowExW, GWL_EXSTYLE, GetClientRect, GetWindowLongPtrW, GetWindowRect,
            GetWindowThreadProcessId, IsWindowVisible, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
            WS_EX_LAYERED, WS_EX_TRANSPARENT,
        },
    },
    core::{PCWSTR, w},
};

pub struct PreparedWindow {
    pub visible: bool,
    pub mouse_passthrough: bool,
}

fn find_overlay() -> Option<HWND> {
    let mut previous = None;
    loop {
        // SAFETY: Only searches by exact title; it does not query unrelated
        // applications synchronously. PID validation excludes other copies.
        let Ok(hwnd) =
            (unsafe { FindWindowExW(None, previous, PCWSTR::null(), w!("Overlay Timer")) })
        else {
            return None;
        };
        let mut process_id = 0;
        // SAFETY: Live enumerated HWND and valid output pointer.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        if process_id == std::process::id() {
            return Some(hwnd);
        }
        previous = Some(hwnd);
    }
}

/// [x, y, width, height], all in physical desktop pixels, independent of DPI.
fn inset_rect(monitor: &crate::monitors::MonitorInfo) -> Result<[i32; 4], String> {
    if monitor.width < 3 || monitor.height < 3 {
        return Err("Der ausgewählte Monitor hat keine gültige Größe.".into());
    }
    Ok([
        monitor.x + 1,
        monitor.y + 1,
        monitor.width - 2,
        monitor.height - 2,
    ])
}

fn outer_rect(hwnd: HWND) -> Result<[i32; 4], String> {
    let mut rect = RECT::default();
    // SAFETY: Caller retains a live own-process HWND, output is a valid RECT.
    unsafe { GetWindowRect(hwnd, &mut rect) }.map_err(|err| err.to_string())?;
    Ok([
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ])
}

fn prepare(hwnd: HWND, monitor_index: usize) -> Result<PreparedWindow, String> {
    let monitors = crate::monitors::enumerate();
    let monitor = monitors.get(monitor_index)
        .ok_or("Der ausgewählte Monitor ist nicht mehr verfügbar. Bitte die Bildschirmliste aktualisieren.")?;
    let target = inset_rect(monitor)?;
    if outer_rect(hwnd)? != target {
        // SAFETY: Sets only the geometry of our own overlay. No activation,
        // z-order change or fullscreen/style transition is requested.
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                target[0],
                target[1],
                target[2],
                target[3],
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }
        .map_err(|err| err.to_string())?;
    }

    let actual = outer_rect(hwnd)?;
    let mut client = RECT::default();
    let mut origin = POINT::default();
    // SAFETY: Valid output buffers and own-process HWND.
    unsafe { GetClientRect(hwnd, &mut client) }.map_err(|err| err.to_string())?;
    if !unsafe { ClientToScreen(hwnd, &mut origin) }.as_bool() {
        return Err("Die Position der Overlay-Zeichenfläche konnte nicht geprüft werden.".into());
    }
    if actual != target
        || origin.x != target[0]
        || origin.y != target[1]
        || client.right - client.left != target[2]
        || client.bottom - client.top != target[3]
    {
        return Err(format!(
            "Overlay-Geometrie stimmt nicht: Soll {target:?}, Ist {actual:?}, \
             Zeichenfläche {client:?} ab {origin:?}."
        ));
    }

    let border_color = DWMWA_COLOR_NONE;
    // SAFETY: DWMWA_BORDER_COLOR expects a 4-byte COLORREF. This does not change
    // geometry or shadow state; it retains the existing border suppression.
    let border_result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            (&raw const border_color).cast::<c_void>(),
            size_of::<u32>() as u32,
        )
    };
    crate::window_trace::record(
        hwnd,
        "one-pixel",
        &format!("target={target:?} dwm_border_none={border_result:?}"),
    );
    // SAFETY: Read-only queries on our existing overlay. winit represents
    // mouse passthrough with both of these extended window-style bits.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    let passthrough_mask = (WS_EX_LAYERED | WS_EX_TRANSPARENT).0;
    Ok(PreparedWindow {
        visible: unsafe { IsWindowVisible(hwnd) }.as_bool(),
        mouse_passthrough: style & passthrough_mask == passthrough_mask,
    })
}

/// None means the deferred viewport has not been created yet. The caller
/// keeps it hidden, then retries from App::logic even if the controller is hidden.
pub fn prepare_overlay(monitor_index: usize) -> Result<Option<PreparedWindow>, String> {
    let Some(hwnd) = find_overlay() else {
        return Ok(None);
    };
    match prepare(hwnd, monitor_index) {
        Ok(window) => Ok(Some(window)),
        Err(error) => {
            crate::window_trace::record(hwnd, "one-pixel", &format!("ERROR: {error}"));
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_stays_inside_negative_and_offset_monitors() {
        for (x, y, width, height) in [(-2560, -1440, 2560, 1440), (2560, 243, 1920, 1080)] {
            let monitor = crate::monitors::MonitorInfo {
                x,
                y,
                width,
                height,
                primary: false,
            };
            let [left, top, w, h] = inset_rect(&monitor).unwrap();
            assert_eq!(
                [
                    left - x,
                    top - y,
                    x + width - (left + w),
                    y + height - (top + h)
                ],
                [1; 4]
            );
        }
    }

    #[test]
    fn unavailable_monitor_never_becomes_a_fullscreen_fallback() {
        let monitor = crate::monitors::MonitorInfo {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            primary: true,
        };
        assert!(inset_rect(&monitor).is_err());
    }
}
