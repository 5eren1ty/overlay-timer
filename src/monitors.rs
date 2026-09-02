use std::mem::size_of;

use windows::{
    Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
        },
    },
    core::BOOL,
};

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

impl MonitorInfo {
    pub fn label(&self, index: usize) -> String {
        let primary = if self.primary { " · primär" } else { "" };
        format!(
            "Display {} · {}×{}{}",
            index + 1,
            self.width,
            self.height,
            primary
        )
    }
}

pub fn enumerate() -> Vec<MonitorInfo> {
    let mut result = Vec::new();

    // SAFETY: `result` remains alive and exclusively borrowed during the synchronous
    // EnumDisplayMonitors call. The callback reconstructs exactly that pointer.
    unsafe {
        let data = LPARAM((&raw mut result).cast::<()>() as isize);
        let _ = EnumDisplayMonitors(None, None, Some(monitor_callback), data);
    }

    if result.is_empty() {
        result.push(MonitorInfo {
            width: 0,
            height: 0,
            primary: true,
        });
    }

    result
}

unsafe extern "system" fn monitor_callback(
    monitor: HMONITOR,
    _device_context: HDC,
    _monitor_rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(data.0 as *mut Vec<MonitorInfo>) };
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

    let info_pointer = (&raw mut info).cast::<MONITORINFO>();
    if !unsafe { GetMonitorInfoW(monitor, info_pointer) }.as_bool() {
        return false.into();
    }

    let rect = info.monitorInfo.rcMonitor;

    monitors.push(MonitorInfo {
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
        primary: info.monitorInfo.dwFlags & 1 != 0,
    });

    true.into()
}
