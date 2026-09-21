//! Native measurements only. No window styles, geometry, focus or pixels change.
use std::{
    fs::{File, OpenOptions},
    io::Write,
    mem::size_of,
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::{
        Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
        Gdi::ClientToScreen,
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetForegroundWindow, GetWindowLongPtrW,
        GetWindowRect, IsWindowVisible,
    },
};

struct Trace {
    file: Option<File>,
    previous: String,
    started: Instant,
}

pub fn record(hwnd: HWND, variant: &str, result: &str) {
    static TRACE: OnceLock<Mutex<Trace>> = OnceLock::new();
    let trace = TRACE.get_or_init(|| {
        let directory = std::env::temp_dir().join("overlay-timer-diagnostics");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let file = std::fs::create_dir_all(&directory).ok().and_then(|()| {
            OpenOptions::new()
                .create_new(true)
                .append(true)
                .open(directory.join(format!(
                    "application-{variant}-{stamp}-{}.log",
                    std::process::id()
                )))
                .ok()
        });
        Mutex::new(Trace {
            file,
            previous: String::new(),
            started: Instant::now(),
        })
    });
    let mut trace = trace
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if trace.file.is_none() {
        return;
    }
    let mut outer = RECT::default();
    let mut client = RECT::default();
    let mut origin = POINT::default();
    let mut bounds = RECT::default();
    // SAFETY: The caller passes a live HWND belonging to this process. All
    // output objects are valid for these synchronous, read-only Win32 calls.
    let state = unsafe {
        let outer_result = GetWindowRect(hwnd, &mut outer);
        let client_result = GetClientRect(hwnd, &mut client);
        let origin_result = ClientToScreen(hwnd, &mut origin);
        let bounds_result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut bounds).cast(),
            size_of::<RECT>() as u32,
        );
        format!(
            "variant={variant} hwnd={:?} visible={} focus={} outer={outer:?} ({outer_result:?}) \
             client={client:?} ({client_result:?}) origin={origin:?} ({origin_result:?}) \
             dwm_frame={bounds:?} ({bounds_result:?}) style=0x{:X} ex_style=0x{:X} {result}",
            hwnd.0,
            IsWindowVisible(hwnd).as_bool(),
            GetForegroundWindow() == hwnd,
            GetWindowLongPtrW(hwnd, GWL_STYLE),
            GetWindowLongPtrW(hwnd, GWL_EXSTYLE),
        )
    };
    if state != trace.previous {
        let seconds = trace.started.elapsed().as_secs_f64();
        if let Some(file) = &mut trace.file {
            let _ = writeln!(file, "t={seconds:.3}s {state}");
        }
        trace.previous = state;
    }
}
