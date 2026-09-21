use std::mem::size_of;

use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::{
        Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_EXTENDED_FRAME_BOUNDS,
            DWMWINDOWATTRIBUTE, DwmGetWindowAttribute, DwmSetWindowAttribute,
        },
        Gdi::{ClientToScreen, RDW_FRAME, RDW_INVALIDATE, RedrawWindow},
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetWindowLongPtrW, GetWindowRect, SWP_NOACTIVATE,
        SWP_NOZORDER, SetWindowPos,
    },
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

use super::{Config, Geometry};

fn hwnd(window: &Window) -> Result<HWND, String> {
    match window
        .window_handle()
        .map_err(|err| err.to_string())?
        .as_raw()
    {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err("Kein Win32-Fensterhandle".to_owned()),
    }
}

/// Physical outer-window coordinates, independent of DPI and egui zoom.
pub fn target_rect(config: &Config) -> [i32; 4] {
    let [x, y, width, height] = config.monitor.rect;
    match config.geometry {
        Geometry::Small => {
            let width = width.min(800);
            let height = height.min(450);
            [
                x + 40.min((config.monitor.rect[2] - width) / 2),
                y + 40.min((config.monitor.rect[3] - height) / 2),
                width,
                height,
            ]
        }
        Geometry::Inset => [x + 1, y + 1, width - 2, height - 2],
        Geometry::Exact | Geometry::Fullscreen => [x, y, width, height],
    }
}

pub fn place(window: &Window, config: &Config) -> Result<(), String> {
    let hwnd = hwnd(window)?;
    let [x, y, width, height] = target_rect(config);
    // SAFETY: Valid HWND from the live winit Window on its owning UI thread.
    // No frame/style mutation: only the explicitly requested physical geometry.
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
    }
    .map_err(|err| err.to_string())
}

fn validate_geometry(
    config: &Config,
    actual: [i32; 4],
    fullscreen: bool,
) -> Result<String, String> {
    let expected = target_rect(config);
    if actual != expected || fullscreen != (config.geometry == Geometry::Fullscreen) {
        return Err(format!(
            "expected_outer={expected:?}, actual_outer={actual:?}, \
             expected_mode={:?}, actual_fullscreen={fullscreen}",
            config.geometry,
        ));
    }
    Ok(format!(
        "Größe geprüft: {} × {} Pixel",
        actual[2], actual[3]
    ))
}

pub fn check_geometry(window: &Window, config: &Config) -> Result<String, String> {
    let hwnd = hwnd(window)?;
    let mut rect = RECT::default();
    // SAFETY: A live window on the owning thread and a writable RECT.
    unsafe { GetWindowRect(hwnd, &mut rect) }.map_err(|err| err.to_string())?;
    validate_geometry(
        config,
        [
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
        ],
        window.fullscreen().is_some(),
    )
}

fn set_color(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, color: u32) -> String {
    // SAFETY: DWM expects a DWORD/COLORREF (4 bytes) for both color attributes.
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute,
            (&raw const color).cast(),
            size_of::<u32>() as u32,
        )
    };
    let mut readback = 0u32;
    // SAFETY: Output points to a writable DWORD with the documented size.
    let read_result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            attribute,
            (&raw mut readback).cast(),
            size_of::<u32>() as u32,
        )
    };
    format!(
        "requested=0x{color:08X}, set={}, read={}, value=0x{readback:08X}",
        result.map_or_else(
            |err| format!("{err} (HRESULT {:?})", err.code()),
            |_| "OK".into()
        ),
        read_result.map_or_else(
            |err| format!("{err} (HRESULT {:?})", err.code()),
            |_| "OK".into()
        ),
    )
}

pub fn apply_colors(window: &Window, config: &Config, redraw: bool) -> String {
    let Ok(hwnd) = hwnd(window) else {
        return "Kein HWND".into();
    };
    let border = set_color(hwnd, DWMWA_BORDER_COLOR, config.border.value());
    let caption = config.caption.value().map_or_else(
        || "unchanged".to_owned(),
        |color| set_color(hwnd, DWMWA_CAPTION_COLOR, color),
    );
    if redraw {
        // SAFETY: Redraw only this diagnostic window's frame, without forcing
        // WM_NCCALCSIZE or changing the native composition policy.
        unsafe {
            let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE | RDW_FRAME);
        }
    }
    format!("border: {border}; caption: {caption}")
}

pub fn snapshot(window: &Window) -> String {
    let Ok(hwnd) = hwnd(window) else {
        return "Kein HWND".into();
    };
    let mut outer = RECT::default();
    let mut client = RECT::default();
    let mut origin = POINT::default();
    let mut frame = RECT::default();
    // SAFETY: All output structs are valid for synchronous Win32 calls and the
    // HWND belongs to the live diagnostic window on this thread.
    let (outer_result, client_result, origin_result, frame_result, style, ex_style) = unsafe {
        (
            GetWindowRect(hwnd, &mut outer),
            GetClientRect(hwnd, &mut client),
            ClientToScreen(hwnd, &mut origin).as_bool(),
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&raw mut frame).cast(),
                size_of::<RECT>() as u32,
            ),
            GetWindowLongPtrW(hwnd, GWL_STYLE),
            GetWindowLongPtrW(hwnd, GWL_EXSTYLE),
        )
    };
    format!(
        "focus={} scale={} fullscreen={:?} outer={outer:?} ({outer_result:?}) \
         client={client:?} ({client_result:?}) client_origin={origin:?} ({origin_result}) \
         dwm_frame={frame:?} ({frame_result:?}) style=0x{style:X} ex_style=0x{ex_style:X}",
        window.has_focus(),
        window.scale_factor(),
        window.fullscreen(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_is_one_physical_pixel_on_every_edge_on_negative_monitor() {
        let mut config = Config::default();
        config.monitor.rect = [-2560, -1440, 2560, 1440];
        config.monitor.scale = 1.5;
        config.geometry = Geometry::Inset;
        assert_eq!(target_rect(&config), [-2559, -1439, 2558, 1438]);
    }

    #[test]
    fn rejects_the_recorded_small_window_with_fullscreen_flag() {
        let mut config = Config::default();
        config.monitor.rect = [2560, 243, 1920, 1080];
        // Reproduction from the user's first diagnostic logs.
        assert!(validate_geometry(&config, [2560, 243, 800, 450], true).is_err());
        assert!(validate_geometry(&config, [2560, 243, 1920, 1080], true).is_ok());
        assert!(validate_geometry(&config, [2560, 243, 1920, 1080], false).is_err());
        assert!(validate_geometry(&config, [0, 0, 1920, 1080], true).is_err());
    }

    #[test]
    fn small_probe_fits_a_small_offset_monitor() {
        let mut config = Config::default();
        config.monitor.rect = [1920, 200, 640, 360];
        config.geometry = Geometry::Small;
        assert_eq!(target_rect(&config), [1920, 200, 640, 360]);
    }
}
