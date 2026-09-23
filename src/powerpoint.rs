//! Read-only observation of a local PowerPoint slide show.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::Instant,
};

use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Com::{
                CLSIDFromProgID, COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize,
                DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPPARAMS, IDispatch,
            },
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Ole::GetActiveObject,
            Variant::{VARIANT, VT_DISPATCH, VT_I4, VariantClear},
        },
        UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
            PeekMessageW, QS_ALLINPUT, TranslateMessage, WM_QUIT,
        },
    },
    core::{Error, GUID, HRESULT, IUnknown, Interface, PCWSTR, Result as WinResult, w},
};

const POLL_MILLIS: u32 = 250;
const UNEXPECTED: HRESULT = HRESULT(0x8000_FFFFu32 as i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorStatus {
    Unavailable,
    Waiting,
    ExistingShow,
    Showing,
    MultipleShows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorEvent {
    Status(MonitorStatus),
    ShowStarted,
    ShowEnded,
}

#[derive(Debug, Clone, Copy)]
pub struct TimedEvent {
    pub event: MonitorEvent,
    pub observed_at: Instant,
}

pub struct PowerPointMonitor {
    receiver: Receiver<TimedEvent>,
    stop: Arc<AtomicBool>,
}

impl PowerPointMonitor {
    pub fn start() -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        thread::Builder::new()
            .name("powerpoint-monitor".into())
            .spawn(move || run_monitor(sender, worker_stop))?;
        Ok(Self { receiver, stop })
    }

    pub fn drain(&self) -> Vec<TimedEvent> {
        self.receiver.try_iter().collect()
    }
}

impl Drop for PowerPointMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> WinResult<Self> {
        // SAFETY: This worker thread has no previous COM apartment. Its lifetime
        // is paired with CoUninitialize in Drop.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: This runs on the same worker thread that initialized COM.
        unsafe { CoUninitialize() };
    }
}

struct AutomationValue(VARIANT);

impl AutomationValue {
    fn integer(value: i32) -> Self {
        let mut variant = VARIANT::default();
        // SAFETY: VT_I4 selects the lVal member of this newly initialized VARIANT.
        unsafe {
            let inner = &mut *variant.Anonymous.Anonymous;
            inner.vt = VT_I4;
            inner.Anonymous.lVal = value;
        }
        Self(variant)
    }

    fn as_integer(&self) -> WinResult<i32> {
        // SAFETY: The tag is checked before reading the union member.
        unsafe {
            let inner = &*self.0.Anonymous.Anonymous;
            if inner.vt != VT_I4 {
                return Err(Error::new(UNEXPECTED, "PowerPoint returned a non-integer"));
            }
            Ok(inner.Anonymous.lVal)
        }
    }

    fn as_dispatch(&self) -> WinResult<IDispatch> {
        // SAFETY: The tag is checked before reading the union member. Cloning
        // AddRefs the interface before VariantClear releases the original.
        unsafe {
            let inner = &*self.0.Anonymous.Anonymous;
            if inner.vt != VT_DISPATCH {
                return Err(Error::new(UNEXPECTED, "PowerPoint returned a non-object"));
            }
            let value: &Option<IDispatch> = &inner.Anonymous.pdispVal;
            value
                .clone()
                .ok_or_else(|| Error::new(UNEXPECTED, "PowerPoint returned a null object"))
        }
    }
}

impl Drop for AutomationValue {
    fn drop(&mut self) {
        // SAFETY: VARIANT owns any value returned by IDispatch::Invoke.
        let _ = unsafe { VariantClear(&mut self.0) };
    }
}

fn invoke(object: &IDispatch, name: PCWSTR, argument: Option<i32>) -> WinResult<AutomationValue> {
    let mut member_id = 0;
    // SAFETY: The name is a static, null-terminated UTF-16 string and all
    // output pointers remain valid for the synchronous COM call.
    unsafe { object.GetIDsOfNames(&GUID::zeroed(), &name, 1, 0, &mut member_id)? };

    let mut argument = argument.map(AutomationValue::integer);
    let parameters = DISPPARAMS {
        rgvarg: argument
            .as_mut()
            .map_or(std::ptr::null_mut(), |value| &mut value.0),
        cArgs: u32::from(argument.is_some()),
        ..Default::default()
    };
    let mut result = AutomationValue(VARIANT::default());
    let flags = if argument.is_some() {
        DISPATCH_METHOD | DISPATCH_PROPERTYGET
    } else {
        DISPATCH_PROPERTYGET
    };
    // SAFETY: The argument and result VARIANTs are alive for Invoke and the
    // DISPPARAMS pointers refer to those stack values.
    unsafe {
        object.Invoke(
            member_id,
            &GUID::zeroed(),
            0,
            flags,
            &parameters,
            Some(&mut result.0),
            None,
            None,
        )?;
    }
    Ok(result)
}

fn active_application() -> WinResult<IDispatch> {
    // SAFETY: GetActiveObject only attaches to an already registered instance.
    // It does not launch PowerPoint.
    unsafe {
        let class_id = CLSIDFromProgID(w!("PowerPoint.Application"))?;
        let mut unknown = None;
        GetActiveObject(&class_id, None, &mut unknown)?;
        unknown
            .ok_or_else(|| Error::new(UNEXPECTED, "PowerPoint has no active object"))?
            .cast()
    }
}

enum Survey {
    NoShow,
    One(IUnknown),
    Multiple,
}

struct SnapshotHandle(HANDLE);

impl Drop for SnapshotHandle {
    fn drop(&mut self) {
        // SAFETY: This is the handle returned by CreateToolhelp32Snapshot.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn powerpoint_process_running() -> Option<bool> {
    // SAFETY: A process-only snapshot is read-only. The owned handle is closed
    // by SnapshotHandle, including on an early return.
    let snapshot = SnapshotHandle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()? });
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: entry has the required size and remains valid during enumeration.
    if unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_err() {
        return None;
    }
    loop {
        let end = entry
            .szExeFile
            .iter()
            .position(|&character| character == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case("POWERPNT.EXE") {
            return Some(true);
        }
        // SAFETY: A failure here normally means the snapshot is exhausted.
        if unsafe { Process32NextW(snapshot.0, &mut entry) }.is_err() {
            return Some(false);
        }
    }
}

fn survey(application: &IDispatch) -> WinResult<Survey> {
    let windows = invoke(application, w!("SlideShowWindows"), None)?.as_dispatch()?;
    let count = invoke(&windows, w!("Count"), None)?.as_integer()?;
    match count {
        0 => Ok(Survey::NoShow),
        1 => {
            let window = invoke(&windows, w!("Item"), Some(1))?.as_dispatch()?;
            Ok(Survey::One(window.cast()?))
        }
        2.. => Ok(Survey::Multiple),
        _ => Err(Error::new(
            UNEXPECTED,
            "PowerPoint returned a negative count",
        )),
    }
}

#[derive(Default)]
struct TransitionTracker {
    initialized: bool,
    show_identity: Option<usize>,
    started_here: bool,
}

impl TransitionTracker {
    fn observe(&mut self, identity: Option<usize>) -> Vec<MonitorEvent> {
        let mut events = Vec::new();
        if !self.initialized {
            self.initialized = true;
            self.show_identity = identity;
            events.push(MonitorEvent::Status(if identity.is_some() {
                MonitorStatus::ExistingShow
            } else {
                MonitorStatus::Waiting
            }));
            return events;
        }
        if self.show_identity != identity {
            if self.show_identity.is_some() {
                events.push(MonitorEvent::ShowEnded);
            }
            if identity.is_some() {
                events.push(MonitorEvent::ShowStarted);
            }
            self.show_identity = identity;
            self.started_here = identity.is_some();
        }
        events.push(MonitorEvent::Status(match identity {
            None => MonitorStatus::Waiting,
            Some(_) if self.started_here => MonitorStatus::Showing,
            Some(_) => MonitorStatus::ExistingShow,
        }));
        events
    }
}

fn send_events(
    events: impl IntoIterator<Item = MonitorEvent>,
    last_status: &mut Option<MonitorStatus>,
    sender: &mpsc::Sender<TimedEvent>,
) -> bool {
    let observed_at = Instant::now();
    for event in events {
        if let MonitorEvent::Status(status) = event {
            if *last_status == Some(status) {
                continue;
            }
            *last_status = Some(status);
        }
        if sender.send(TimedEvent { event, observed_at }).is_err() {
            return false;
        }
    }
    true
}

fn run_monitor(sender: mpsc::Sender<TimedEvent>, stop: Arc<AtomicBool>) {
    let Ok(_apartment) = ComApartment::initialize() else {
        let _ = sender.send(TimedEvent {
            event: MonitorEvent::Status(MonitorStatus::Unavailable),
            observed_at: Instant::now(),
        });
        return;
    };
    let mut application: Option<IDispatch> = None;
    let mut retained_window: Option<IUnknown> = None;
    let mut tracker = TransitionTracker::default();
    let mut last_status = None;
    while !stop.load(Ordering::Acquire) {
        if application.is_none() {
            application = active_application().ok();
        }
        let observation = application.as_ref().map(survey);
        match observation {
            Some(Ok(Survey::NoShow)) => {
                if !send_events(tracker.observe(None), &mut last_status, &sender) {
                    return;
                }
                retained_window = None;
            }
            Some(Ok(Survey::One(window))) => {
                let identity = window.as_raw() as usize;
                if !send_events(tracker.observe(Some(identity)), &mut last_status, &sender) {
                    return;
                }
                retained_window = Some(window);
            }
            Some(Ok(Survey::Multiple)) => {
                if !send_events(
                    [MonitorEvent::Status(MonitorStatus::MultipleShows)],
                    &mut last_status,
                    &sender,
                ) {
                    return;
                }
            }
            Some(Err(_)) => {
                application = None;
                if !report_disconnection(&mut tracker, &mut last_status, &sender) {
                    return;
                }
            }
            None => {
                if !report_disconnection(&mut tracker, &mut last_status, &sender) {
                    return;
                }
            }
        }
        pump_messages();
    }
    drop(retained_window);
}

fn report_disconnection(
    tracker: &mut TransitionTracker,
    last_status: &mut Option<MonitorStatus>,
    sender: &mpsc::Sender<TimedEvent>,
) -> bool {
    if powerpoint_process_running() == Some(false) {
        send_events(tracker.observe(None), last_status, sender)
    } else {
        send_events(
            [MonitorEvent::Status(MonitorStatus::Unavailable)],
            last_status,
            sender,
        )
    }
}

fn pump_messages() {
    // SAFETY: This STA owns its thread's message queue. We dispatch only
    // messages addressed to it and wait for new input or the next poll.
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            if message.message == WM_QUIT {
                return;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        MsgWaitForMultipleObjectsEx(None, POLL_MILLIS, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
}

#[cfg(test)]
mod tests {
    use super::{MonitorEvent, MonitorStatus, TransitionTracker};

    #[test]
    fn existing_show_is_not_reported_as_a_new_start() {
        let mut tracker = TransitionTracker::default();
        assert_eq!(
            tracker.observe(Some(1)),
            vec![MonitorEvent::Status(MonitorStatus::ExistingShow)]
        );
        assert_eq!(
            tracker.observe(Some(1)),
            vec![MonitorEvent::Status(MonitorStatus::ExistingShow)]
        );
    }

    #[test]
    fn successive_show_identities_start_distinct_runs() {
        let mut tracker = TransitionTracker::default();
        assert_eq!(
            tracker.observe(None),
            vec![MonitorEvent::Status(MonitorStatus::Waiting)]
        );
        assert_eq!(
            tracker.observe(Some(1)),
            vec![
                MonitorEvent::ShowStarted,
                MonitorEvent::Status(MonitorStatus::Showing)
            ]
        );
        assert_eq!(
            tracker.observe(Some(1)),
            vec![MonitorEvent::Status(MonitorStatus::Showing)]
        );
        assert_eq!(
            tracker.observe(Some(2)),
            vec![
                MonitorEvent::ShowEnded,
                MonitorEvent::ShowStarted,
                MonitorEvent::Status(MonitorStatus::Showing)
            ]
        );
        assert_eq!(
            tracker.observe(None),
            vec![
                MonitorEvent::ShowEnded,
                MonitorEvent::Status(MonitorStatus::Waiting)
            ]
        );
    }
}
