//! Standalone, manually operated reproduction harness. No production modules are
//! patched or loaded; each experiment runs in its own disposable child process.
mod native;

use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eframe::egui::{self, Color32, RichText};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use serde::{Deserialize, Serialize};
use winit::{platform::windows::WindowExtWindows, window::Fullscreen};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Monitor {
    index: usize,
    name: String,
    rect: [i32; 4],
    scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Geometry {
    Small,
    Fullscreen,
    Exact,
    Inset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Border {
    None,
    Black,
    Red,
    Default,
}

impl Border {
    fn value(self) -> u32 {
        match self {
            Self::None => 0xFFFF_FFFE,
            Self::Default => 0xFFFF_FFFF,
            Self::Black => 0,
            Self::Red => 0x0000_00FF, // COLORREF is 0x00BBGGRR, without alpha.
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Caption {
    Unchanged,
    Black,
    Green,
}

impl Caption {
    fn value(self) -> Option<u32> {
        match self {
            Self::Unchanged => None,
            Self::Black => Some(0),
            Self::Green => Some(0x0000_FF00),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum ClientLine {
    None,
    Black,
    Blue,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Config {
    monitor: Monitor,
    geometry: Geometry,
    border: Border,
    caption: Caption,
    client_line: ClientLine,
    shadow: bool,
    passthrough: bool,
    focus: bool,
    renderer_alpha: bool,
    seconds: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor: Monitor {
                index: 0,
                name: "Monitor wird ermittelt".into(),
                rect: [0, 0, 1920, 1080],
                scale: 1.0,
            },
            geometry: Geometry::Fullscreen,
            border: Border::None,
            caption: Caption::Unchanged,
            client_line: ClientLine::None,
            shadow: true,
            passthrough: true,
            focus: false,
            // eframe's production root is opaque, although its overlay viewport
            // is transparent. Keep the global Painter flag equal by default.
            renderer_alpha: false,
            seconds: 20,
        }
    }
}

fn graphics_options() -> eframe::WgpuConfiguration {
    let mut options = eframe::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::GL;
    }
    options
}

fn app_error(message: impl Into<String>) -> eframe::Error {
    eframe::Error::AppCreation(message.into().into())
}

pub fn run() -> eframe::Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--help") {
        println!(
            "Overlay-Diagnose: ohne Argumente die Steuerung starten. \
                  Protokolle unter TEMP/overlay-timer-diagnostics. \
                  Strg+Alt+F12 beendet das laufende Experiment."
        );
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--probe") {
        let config: Config = serde_json::from_str(
            args.get(1)
                .ok_or_else(|| app_error("Testkonfiguration fehlt"))?,
        )
        .map_err(|err| app_error(format!("Ungültige Testkonfiguration: {err}")))?;
        if config.monitor.rect[2] < 3
            || config.monitor.rect[3] < 3
            || !(5..=120).contains(&config.seconds)
        {
            return Err(app_error("Ungültige Monitorgröße oder Testdauer"));
        }
        return run_probe(config);
    }
    if !args.is_empty() {
        return Err(app_error(
            "Unbekannte Argumente; --help zeigt die Bedienung",
        ));
    }
    eframe::run_native(
        "Overlay-Diagnose",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Overlay-Diagnose · Revision 2 · Steuerung")
                .with_app_id("overlay-timer-diagnostics")
                .with_inner_size([670.0, 740.0])
                .with_min_inner_size([560.0, 560.0]),
            persist_window: false,
            renderer: eframe::Renderer::Wgpu,
            wgpu_options: graphics_options(),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::new(Controller::new()))),
    )
}

struct Experiment {
    child: Child,
    started: Instant,
    config: Config,
}

struct Controller {
    config: Config,
    monitors: Vec<Monitor>,
    active: Option<Experiment>,
    log_path: Option<PathBuf>,
    observation: String,
    status: String,
    hotkey: Option<GlobalHotKeyManager>,
    stop_id: u32,
    hotkey_error: Option<String>,
}

impl Controller {
    fn new() -> Self {
        let stop_key = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::F12);
        let registration = GlobalHotKeyManager::new().and_then(|manager| {
            manager.register(stop_key)?;
            Ok(manager)
        });
        let (hotkey, hotkey_error) = match registration {
            Ok(manager) => (Some(manager), None),
            Err(err) => (None, Some(err.to_string())),
        };
        Self {
            config: Config::default(),
            monitors: Vec::new(),
            active: None,
            log_path: None,
            observation: String::new(),
            status: "Bereit. Zuerst Ausgangszustand, Rot und Schwarz vergleichen.".into(),
            hotkey,
            stop_id: stop_key.id(),
            hotkey_error,
        }
    }

    fn stop(&mut self) {
        if let Some(mut experiment) = self.active.take() {
            // Only terminate the disposable diagnostic process started by this
            // controller. There is no application state to save in that process.
            match experiment
                .child
                .kill()
                .and_then(|()| experiment.child.wait())
            {
                Ok(status) => self.status = format!("Test beendet ({status})."),
                Err(err) => self.status = format!("Testende: {err}"),
            }
        }
    }

    fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop();
        let directory = std::env::temp_dir().join("overlay-timer-diagnostics");
        std::fs::create_dir_all(&directory)?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let path = directory.join(format!("test-{stamp}-{}.log", std::process::id()));
        let mut log = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)?;
        let config = self.config.clone();
        let encoded = serde_json::to_string(&config)?;
        writeln!(
            log,
            "overlay-diagnostics v{}; diagnostic_revision=2; unix_ms={stamp}",
            env!("CARGO_PKG_VERSION")
        )?;
        writeln!(log, "config={encoded}")?;
        writeln!(
            log,
            "Same eframe/egui-winit/winit/wgpu-GL versions as the main app. \
                       A separate root window models the overlay; no multi-viewport controller. \
                       Shadow is applied before its first visible frame, after surface creation."
        )?;
        let child = Command::new(std::env::current_exe()?)
            .arg("--probe")
            .arg(encoded)
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        self.active = Some(Experiment {
            child,
            started: Instant::now(),
            config,
        });
        self.log_path = Some(path);
        self.observation.clear();
        self.status = "Test läuft. Strg+Alt+F12 beendet ihn jederzeit.".into();
        Ok(())
    }

    fn poll(&mut self) {
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.stop_id && event.state == HotKeyState::Pressed {
                self.stop();
            }
        }
        if let Some(active) = &mut self.active {
            match active.child.try_wait() {
                Ok(Some(status)) => {
                    self.status = if status.success() {
                        "Test abgeschlossen. Beobachtung kann gespeichert werden.".into()
                    } else {
                        format!("Testprozess beendet: {status}. Details stehen im Protokoll.")
                    };
                    self.active = None;
                }
                Ok(None)
                    if active.started.elapsed().as_secs()
                        > u64::from(active.config.seconds) + 15 =>
                {
                    self.stop();
                    self.status = "Test nach Zeitlimit beendet; Details im Protokoll.".into();
                }
                Err(err) => {
                    self.status = format!("Prozessstatus konnte nicht gelesen werden: {err}")
                }
                _ => {}
            }
        }
    }

    fn save_observation(&mut self) {
        let Some(path) = &self.log_path else { return };
        let result = OpenOptions::new()
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "\nMANUELLE BEOBACHTUNG: {}", self.observation));
        self.status = match result {
            Ok(()) => "Beobachtung gespeichert.".into(),
            Err(err) => format!("Beobachtung konnte nicht gespeichert werden: {err}"),
        };
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.stop();
        // Keep global hotkey registration alive until after the child is stopped.
        self.hotkey.take();
    }
}

impl eframe::App for Controller {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll();
        // eframe skips UI passes for minimized windows, but still runs logic.
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(window) = frame.winit_window() {
            self.monitors = window
                .available_monitors()
                .enumerate()
                .map(|(index, monitor)| {
                    let position = monitor.position();
                    let size = monitor.size();
                    Monitor {
                        index,
                        name: monitor
                            .name()
                            .unwrap_or_else(|| format!("Display {}", index + 1)),
                        rect: [
                            position.x,
                            position.y,
                            size.width as i32,
                            size.height as i32,
                        ],
                        scale: monitor.scale_factor(),
                    }
                })
                .collect();
            if let Some(monitor) = self
                .monitors
                .get(self.config.monitor.index)
                .or_else(|| self.monitors.first())
            {
                self.config.monitor = monitor.clone();
            }
        }
        egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Overlay-Diagnose · Revision 2");
            ui.label("Vergleiche Randfarbe und Transparenz. Jeder Test schließt sich automatisch.");
            ui.label(RichText::new("Sofort beenden: Strg + Alt + F12").strong());
            if let Some(error) = &self.hotkey_error {
                ui.colored_label(Color32::YELLOW,
                    format!("Tastenkürzel nicht verfügbar: {error}. Automatisches Testende bleibt aktiv."));
            }
            ui.separator();
            ui.add_enabled_ui(self.active.is_none(), |ui| {
                egui::ComboBox::from_label("Bildschirm")
                    .selected_text(format!("{} · {} × {}", self.config.monitor.name,
                        self.config.monitor.rect[2], self.config.monitor.rect[3]))
                    .show_ui(ui, |ui| {
                        for monitor in &self.monitors {
                            if ui.selectable_label(self.config.monitor.index == monitor.index,
                                format!("{} · {} × {} · {} %", monitor.name, monitor.rect[2],
                                    monitor.rect[3], (monitor.scale * 100.0).round())).clicked()
                            {
                                self.config.monitor = monitor.clone();
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("Farbvergleich:");
                    for (label, border) in [
                        ("Ausgangszustand", Border::None),
                        ("Schwarz", Border::Black),
                        ("Diagnoserot", Border::Red),
                    ] {
                        if ui.button(label).clicked() {
                            self.config.geometry = Geometry::Fullscreen;
                            self.config.shadow = true;
                            self.config.border = border;
                            self.config.caption = Caption::Unchanged;
                            self.config.client_line = ClientLine::None;
                            self.config.renderer_alpha = false;
                        }
                    }
                });
                combo(ui, "Fenstergröße", &mut self.config.geometry, &[
                    (Geometry::Fullscreen, "Monitorfüllend wie Hauptanwendung"),
                    (Geometry::Small, "Kleines Fenster (bis 800 × 450 Pixel)"),
                    (Geometry::Exact, "Normales Fenster in exakter Monitorgröße"),
                    (Geometry::Inset, "Normales Fenster, jede Kante 1 Pixel eingerückt"),
                ]);
                combo(ui, "Randfarbe", &mut self.config.border, &[
                    (Border::None, "Rand unterdrücken (bisheriger Ansatz)"),
                    (Border::Black, "Schwarz"),
                    (Border::Red, "Rot zur Diagnose"),
                    (Border::Default, "Windows-Standard"),
                ]);
                ui.checkbox(&mut self.config.shadow, "Fensterschatten aktiv");
                ui.checkbox(&mut self.config.passthrough, "Mausklicks durchreichen");
                ui.checkbox(&mut self.config.focus, "Testfenster beim Start fokussieren");
                ui.add(egui::Slider::new(&mut self.config.seconds, 10..=60).text("Sekunden pro Test"));
                ui.collapsing("Weitere Vergleichsmöglichkeiten", |ui| {
                    combo(ui, "Titelleistenfarbe", &mut self.config.caption, &[
                        (Caption::Unchanged, "Unverändert"),
                        (Caption::Black, "Schwarz"),
                        (Caption::Green, "Grün zur Diagnose"),
                    ]);
                    combo(ui, "Erste gezeichnete Pixelzeile", &mut self.config.client_line, &[
                        (ClientLine::None, "Keine zusätzliche Linie"),
                        (ClientLine::Black, "Schwarze Linie"),
                        (ClientLine::Blue, "Blaue Linie zur Abgrenzung"),
                    ]);
                    ui.checkbox(&mut self.config.renderer_alpha,
                        "Transparenten Renderer-Hauptpuffer anfordern");
                    ui.small("Standard aus entspricht der globalen Renderer-Einstellung der Hauptanwendung. Das native Testfenster ist in beiden Fällen transparent angefordert.");
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(self.active.is_none() && !self.monitors.is_empty(),
                    egui::Button::new("Test starten")).clicked()
                    && let Err(err) = self.start()
                {
                    self.status = format!("Start fehlgeschlagen: {err}");
                }
                if ui.add_enabled(self.active.is_some(), egui::Button::new("Test beenden")).clicked() {
                    self.stop();
                }
                if let Some(active) = &self.active {
                    let remaining = u64::from(active.config.seconds)
                        .saturating_sub(active.started.elapsed().as_secs());
                    ui.label(format!("Noch ungefähr {remaining} s"));
                }
            });
            ui.label(&self.status);
            if let Some(path) = &self.log_path {
                ui.small(format!("Protokoll: {}", path.display()));
                if ui.button("Protokollpfad kopieren").clicked() {
                    ctx.copy_text(path.to_string_lossy().into_owned());
                }
                ui.label("Beobachtung: Randfarbe, Transparenz, Fokus, Klickdurchlässigkeit");
                ui.text_edit_multiline(&mut self.observation);
                if ui.add_enabled(self.active.is_none() && !self.observation.trim().is_empty(),
                    egui::Button::new("Beobachtung speichern")).clicked()
                {
                    self.save_observation();
                }
            }
            ui.separator();
            ui.small("Ein erfolgreicher Windows-Aufruf beweist noch keine sichtbare Farbänderung. Bitte Rot und Schwarz getrennt bei unveränderter Geometrie vergleichen.");
        });
        });
    }
}

fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    choices: &[(T, &str)],
) {
    let selected = choices
        .iter()
        .find(|(candidate, _)| candidate == value)
        .map_or("", |(_, text)| *text);
    egui::ComboBox::from_label(label)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for &(candidate, text) in choices {
                ui.selectable_value(value, candidate, text);
            }
        });
}

fn probe_viewport(config: &Config) -> egui::ViewportBuilder {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Overlay-Diagnose · Testfenster")
        .with_app_id("overlay-timer-diagnostics-probe")
        .with_decorations(false)
        .with_transparent(config.renderer_alpha)
        .with_always_on_top()
        .with_mouse_passthrough(config.passthrough)
        .with_taskbar(false)
        .with_active(config.focus)
        .with_resizable(false);
    if config.geometry == Geometry::Fullscreen {
        // egui-winit reapplies inner_size AFTER creating the fullscreen window.
        // An 800x450 fallback here silently shrinks it without clearing the
        // fullscreen flag. Let the target monitor determine the size instead.
        viewport.with_monitor(config.monitor.index)
    } else {
        viewport.with_inner_size([800.0, 450.0])
    }
}

fn finalize_probe_viewport(
    mut viewport: egui::ViewportBuilder,
    fullscreen: bool,
) -> egui::ViewportBuilder {
    if fullscreen {
        // Also discard any geometry restored by eframe before this hook.
        viewport.inner_size = None;
        viewport.position = None;
    }
    viewport.with_transparent(true)
}

fn run_probe(config: Config) -> eframe::Result {
    let failed = Arc::new(AtomicBool::new(false));
    let probe_failed = Arc::clone(&failed);
    let viewport = probe_viewport(&config);
    let fullscreen = config.geometry == Geometry::Fullscreen;
    eframe::run_native(
        "Overlay-Diagnose · Testfenster",
        eframe::NativeOptions {
            viewport,
            // The native window always requests transparency. The separate
            // Painter alpha preference above is false in the production app;
            // changing it is an explicitly isolated diagnostic variable.
            window_builder: Some(Box::new(move |builder| {
                finalize_probe_viewport(builder, fullscreen)
            })),
            persist_window: false,
            renderer: eframe::Renderer::Wgpu,
            wgpu_options: graphics_options(),
            ..Default::default()
        },
        Box::new(move |creation| {
            if let Some(render) = &creation.wgpu_render_state {
                eprintln!("adapter={:?}", render.adapter.get_info());
            }
            Ok(Box::new(Probe {
                failed: probe_failed,
                config,
                started: Instant::now(),
                initialized: false,
                focus_requested: false,
                last_state: String::new(),
                last_colors: String::new(),
                geometry_status: String::new(),
            }))
        }),
    )?;
    if failed.load(Ordering::Relaxed) {
        Err(app_error(
            "Testeinrichtung fehlgeschlagen; siehe ABORT im Protokoll.",
        ))
    } else {
        Ok(())
    }
}

struct Probe {
    failed: Arc<AtomicBool>,
    config: Config,
    started: Instant,
    initialized: bool,
    focus_requested: bool,
    last_state: String,
    last_colors: String,
    geometry_status: String,
}

impl eframe::App for Probe {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.started.elapsed() >= Duration::from_secs(u64::from(self.config.seconds)) {
            eprintln!(
                "normal_close after {:.3}s",
                self.started.elapsed().as_secs_f64()
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(window) = frame.winit_window() {
            if !self.initialized {
                let target = window.available_monitors().find(|monitor| {
                    let pos = monitor.position();
                    let size = monitor.size();
                    [pos.x, pos.y, size.width as i32, size.height as i32]
                        == self.config.monitor.rect
                });
                let Some(target) = target else {
                    eprintln!(
                        "ABORT: Gewählter Monitor ist nicht mehr mit derselben Geometrie verfügbar."
                    );
                    self.failed.store(true, Ordering::Relaxed);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                };
                eprintln!("before_setup: {}", native::snapshot(window));
                window.set_undecorated_shadow(self.config.shadow);
                if self.config.geometry == Geometry::Fullscreen {
                    window.set_fullscreen(Some(Fullscreen::Borderless(Some(target))));
                } else if let Err(err) = native::place(window, &self.config) {
                    eprintln!("ABORT: Fensterpositionierung fehlgeschlagen: {err}");
                    self.failed.store(true, Ordering::Relaxed);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                if let Err(err) = window.set_cursor_hittest(!self.config.passthrough) {
                    eprintln!("ABORT: Klickdurchlässigkeit konnte nicht gesetzt werden: {err}");
                    self.failed.store(true, Ordering::Relaxed);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                self.last_colors = native::apply_colors(window, &self.config, true);
                eprintln!("colors: {}", self.last_colors);
                self.initialized = true;
                self.started = Instant::now();
            } else if self.config.focus && !self.focus_requested {
                window.focus_window();
                self.focus_requested = true;
                eprintln!("focus requested after first frame");
            }
            // The fullscreen flag alone is not proof of monitor coverage.
            // Validate physical geometry before presenting any diagnostic data.
            if !window.is_minimized().unwrap_or(false) {
                match native::check_geometry(window, &self.config) {
                    Ok(status) => {
                        if status != self.geometry_status {
                            eprintln!("geometry_verified: {status}");
                            self.geometry_status = status;
                        }
                    }
                    Err(err) => {
                        eprintln!("ABORT: Ungültige Testgeometrie: {err}");
                        eprintln!("invalid_geometry_state: {}", native::snapshot(window));
                        self.failed.store(true, Ordering::Relaxed);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return;
                    }
                }
            }
            let state = native::snapshot(window);
            let changed = state != self.last_state;
            if changed {
                eprintln!("t={:.3}s {state}", self.started.elapsed().as_secs_f64());
                self.last_state = state;
            }
            // Like the production suppression, repeat after native style/focus
            // changes. Log failures/readback changes rather than every frame.
            let colors = native::apply_colors(window, &self.config, changed);
            if colors != self.last_colors {
                eprintln!(
                    "t={:.3}s colors: {colors}",
                    self.started.elapsed().as_secs_f64()
                );
                self.last_colors = colors;
            }
        }
        let elapsed = self.started.elapsed();
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            eprintln!("normal_close after {:.3}s", elapsed.as_secs_f64());
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let rect = ctx.content_rect();
        let center = rect.center();
        let painter = ui.painter();
        let card = egui::Rect::from_center_size(center, egui::vec2(390.0, 210.0));
        painter.rect_filled(card, 14.0, Color32::from_black_alpha(160));
        painter.text(
            center - egui::vec2(0.0, 40.0),
            egui::Align2::CENTER_CENTER,
            format!(
                "00:{:02}",
                self.config.seconds.saturating_sub(elapsed.as_secs() as u32)
            ),
            egui::FontId::monospace(48.0),
            Color32::WHITE,
        );
        painter.text(
            center + egui::vec2(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            "Der Desktop muss außerhalb sichtbar bleiben.",
            egui::FontId::proportional(15.0),
            Color32::WHITE,
        );
        let x = center.x + (elapsed.as_secs_f32() * 2.0).sin() * 110.0;
        painter.circle_filled(
            egui::pos2(x, center.y + 57.0),
            16.0,
            Color32::from_rgba_unmultiplied(60, 190, 255, 150),
        );
        painter.text(
            center + egui::vec2(0.0, 87.0),
            egui::Align2::CENTER_CENTER,
            &self.geometry_status,
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
        if self.config.client_line != ClientLine::None {
            let color = if self.config.client_line == ClientLine::Black {
                Color32::BLACK
            } else {
                Color32::BLUE
            };
            painter.rect_filled(
                egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(rect.width(), 1.0 / ctx.pixels_per_point()),
                ),
                0.0,
                color,
            );
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_cannot_be_shrunk_by_post_creation_inner_size() {
        let config = Config::default();
        let initial = probe_viewport(&config);
        assert_eq!(initial.monitor, Some(config.monitor.index));
        assert_eq!(initial.inner_size, None);
        // Model restored root-window geometry before eframe calls our hook.
        let restored = initial
            .with_inner_size([800.0, 450.0])
            .with_position([20.0, 30.0]);
        let final_builder = finalize_probe_viewport(restored, true);
        assert_eq!(final_builder.inner_size, None);
        assert_eq!(final_builder.position, None);
        assert_eq!(final_builder.monitor, Some(config.monitor.index));
        assert_eq!(final_builder.transparent, Some(true));
    }
}
