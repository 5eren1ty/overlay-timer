use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, RichText};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use serde::{Deserialize, Serialize};

use crate::{
    monitors::{self, MonitorInfo},
    overlay::{OverlayBridge, OverlayEvent, OverlaySnapshot},
    timer::{CountdownTimer, TimerReading, TimerState, format_reading},
    tray::{TrayCommand, TrayController},
};

const STORAGE_KEY: &str = "overlay_timer_settings";
const MIN_FONT_SIZE: f32 = 32.0;
const MAX_FONT_SIZE: f32 = 128.0;

#[derive(Clone, Copy)]
struct Palette {
    canvas: Color32,
    surface: Color32,
    surface_high: Color32,
    outline: Color32,
    primary: Color32,
    primary_fill: Color32,
    text: Color32,
    muted: Color32,
    success: Color32,
    success_bg: Color32,
    paused: Color32,
    paused_bg: Color32,
    error: Color32,
    error_bg: Color32,
}

impl Palette {
    fn for_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self {
                canvas: Color32::from_rgb(11, 15, 23),
                surface: Color32::from_rgb(21, 27, 38),
                surface_high: Color32::from_rgb(29, 38, 51),
                outline: Color32::from_rgb(45, 56, 73),
                primary: Color32::from_rgb(139, 180, 255),
                primary_fill: Color32::from_rgb(47, 111, 237),
                text: Color32::from_rgb(243, 246, 252),
                muted: Color32::from_rgb(151, 163, 183),
                success: Color32::from_rgb(92, 224, 160),
                success_bg: Color32::from_rgb(21, 55, 42),
                paused: Color32::from_rgb(255, 209, 102),
                paused_bg: Color32::from_rgb(58, 49, 26),
                error: Color32::from_rgb(255, 123, 114),
                error_bg: Color32::from_rgb(66, 29, 33),
            }
        } else {
            Self {
                canvas: Color32::from_rgb(246, 247, 250),
                surface: Color32::WHITE,
                surface_high: Color32::from_rgb(238, 241, 246),
                outline: Color32::from_rgb(215, 220, 229),
                primary: Color32::from_rgb(37, 99, 235),
                primary_fill: Color32::from_rgb(37, 99, 235),
                text: Color32::from_rgb(24, 33, 47),
                muted: Color32::from_rgb(102, 112, 133),
                success: Color32::from_rgb(22, 121, 75),
                success_bg: Color32::from_rgb(221, 245, 232),
                paused: Color32::from_rgb(138, 90, 0),
                paused_bg: Color32::from_rgb(255, 240, 194),
                error: Color32::from_rgb(179, 38, 30),
                error_bg: Color32::from_rgb(249, 222, 220),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl OverlayPosition {
    const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Oben links",
            Self::TopRight => "Oben rechts",
            Self::BottomLeft => "Unten links",
            Self::BottomRight => "Unten rechts",
        }
    }

    fn anchor(self, margin: f32) -> (Align2, egui::Vec2) {
        match self {
            Self::TopLeft => (Align2::LEFT_TOP, egui::vec2(margin, margin)),
            Self::TopRight => (Align2::RIGHT_TOP, egui::vec2(-margin, margin)),
            Self::BottomLeft => (Align2::LEFT_BOTTOM, egui::vec2(margin, -margin)),
            Self::BottomRight => (Align2::RIGHT_BOTTOM, egui::vec2(-margin, -margin)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    duration_seconds: u64,
    overlay_visible: bool,
    monitor_index: usize,
    position: OverlayPosition,
    margin: f32,
    font_size: f32,
    text_color: [u8; 3],
    background_opacity: u8,
    custom_position: Option<[f32; 2]>,
    dark_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            duration_seconds: 10 * 60,
            overlay_visible: true,
            monitor_index: 0,
            position: OverlayPosition::BottomRight,
            margin: 36.0,
            font_size: 64.0,
            text_color: [255, 255, 255],
            background_opacity: 190,
            custom_position: None,
            dark_mode: true,
        }
    }
}

struct Hotkeys {
    _manager: Option<GlobalHotKeyManager>,
    toggle_timer_id: u32,
    reset_id: u32,
    toggle_overlay_id: u32,
    error: Option<String>,
}

impl Hotkeys {
    fn register() -> Self {
        let modifiers = Modifiers::CONTROL | Modifiers::ALT;
        let toggle_timer = HotKey::new(Some(modifiers), Code::KeyP);
        let reset = HotKey::new(Some(modifiers), Code::KeyR);
        let toggle_overlay = HotKey::new(Some(modifiers), Code::KeyO);

        match GlobalHotKeyManager::new() {
            Ok(manager) => {
                let mut errors = Vec::new();
                for (label, hotkey) in [
                    ("Ctrl+Alt+P", toggle_timer),
                    ("Ctrl+Alt+R", reset),
                    ("Ctrl+Alt+O", toggle_overlay),
                ] {
                    if let Err(error) = manager.register(hotkey) {
                        errors.push(format!("{label}: {error}"));
                    }
                }
                Self {
                    _manager: Some(manager),
                    toggle_timer_id: toggle_timer.id(),
                    reset_id: reset.id(),
                    toggle_overlay_id: toggle_overlay.id(),
                    error: (!errors.is_empty()).then(|| errors.join("; ")),
                }
            }
            Err(error) => Self {
                _manager: None,
                toggle_timer_id: toggle_timer.id(),
                reset_id: reset.id(),
                toggle_overlay_id: toggle_overlay.id(),
                error: Some(error.to_string()),
            },
        }
    }
}

pub struct OverlayTimerApp {
    settings: Settings,
    timer: CountdownTimer,
    monitors: Vec<MonitorInfo>,
    hotkeys: Hotkeys,
    tray: Option<TrayController>,
    tray_error: Option<String>,
    overlay: OverlayBridge,
    edit_mode: bool,
    control_hidden: bool,
    theme_initialized_after_startup: bool,
}

impl OverlayTimerApp {
    pub fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        let settings: Settings = creation_context
            .storage
            .and_then(|storage| eframe::get_value(storage, STORAGE_KEY))
            .unwrap_or_default();
        configure_theme(&creation_context.egui_ctx, settings.dark_mode);

        let timer = CountdownTimer::new(Duration::from_secs(settings.duration_seconds));
        let overlay =
            OverlayBridge::new(overlay_snapshot(&settings, &timer, false, Instant::now()));
        let (tray, tray_error) = match TrayController::new() {
            Ok(tray) => (Some(tray), None),
            Err(error) => (None, Some(error)),
        };
        let mut app = Self {
            settings,
            timer,
            monitors: monitors::enumerate(),
            hotkeys: Hotkeys::register(),
            tray,
            tray_error,
            overlay,
            edit_mode: false,
            control_hidden: false,
            theme_initialized_after_startup: false,
        };
        app.clamp_monitor_index();
        app.sync_tray(Instant::now());
        app
    }

    fn clamp_monitor_index(&mut self) {
        self.settings.monitor_index = self
            .settings
            .monitor_index
            .min(self.monitors.len().saturating_sub(1));
    }

    fn set_duration(&mut self, duration_seconds: u64) {
        if self.timer.is_running() {
            return;
        }
        self.settings.duration_seconds = duration_seconds;
        self.timer.set_total(Duration::from_secs(duration_seconds));
    }

    fn toggle_timer(&mut self, now: Instant) {
        if self.settings.duration_seconds == 0 && !self.timer.reading(now).is_overtime() {
            return;
        }
        self.timer.toggle(now);
        if self.timer.is_running() {
            self.edit_mode = false;
        }
    }

    fn toggle_overlay(&mut self) {
        self.settings.overlay_visible = !self.settings.overlay_visible;
        if !self.settings.overlay_visible {
            self.edit_mode = false;
        }
    }

    fn process_hotkeys(&mut self, now: Instant) {
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state != HotKeyState::Pressed {
                continue;
            }
            if event.id == self.hotkeys.toggle_timer_id {
                self.toggle_timer(now);
            } else if event.id == self.hotkeys.reset_id {
                self.timer.reset();
            } else if event.id == self.hotkeys.toggle_overlay_id {
                self.toggle_overlay();
            }
        }
    }

    fn process_tray(&mut self, ctx: &egui::Context, now: Instant) {
        let commands = self
            .tray
            .as_ref()
            .map_or_else(Vec::new, TrayController::drain_commands);
        for command in commands {
            match command {
                TrayCommand::OpenControl => self.restore_control(ctx),
                TrayCommand::ToggleTimer => self.toggle_timer(now),
                TrayCommand::Reset => self.timer.reset(),
                TrayCommand::ToggleOverlay => self.toggle_overlay(),
                TrayCommand::Exit => {
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close)
                }
            }
        }
    }

    fn process_overlay_events(&mut self) {
        for event in self.overlay.drain_events() {
            match event {
                OverlayEvent::PositionChanged(position) => {
                    self.settings.custom_position = Some(position);
                }
                OverlayEvent::TransformChanged {
                    position,
                    font_size,
                } => {
                    self.settings.custom_position = Some(position);
                    self.settings.font_size = font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
                }
            }
        }
    }

    fn handle_minimize(&mut self, ctx: &egui::Context) {
        let minimized = ctx.input(|input| input.viewport().minimized) == Some(true);
        if minimized && !self.control_hidden && self.tray.is_some() {
            self.edit_mode = false;
            self.control_hidden = true;
            ctx.send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::Visible(false),
            );
            ctx.send_viewport_cmd_to(
                OverlayBridge::viewport_id(),
                egui::ViewportCommand::MousePassthrough(true),
            );
        }
    }

    fn restore_control(&mut self, ctx: &egui::Context) {
        self.control_hidden = false;
        for command in [
            egui::ViewportCommand::Visible(true),
            egui::ViewportCommand::Minimized(false),
            egui::ViewportCommand::Focus,
        ] {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, command);
        }
    }

    fn sync_tray(&self, now: Instant) {
        if let Some(tray) = &self.tray {
            let reading = self.timer.reading(now);
            tray.sync(
                self.timer.is_running(),
                reading.is_overtime(),
                self.timer.is_running()
                    || reading.is_overtime()
                    || self.settings.duration_seconds > 0,
                self.settings.overlay_visible,
            );
        }
    }

    fn sync_overlay(&self, now: Instant) {
        self.overlay.update(overlay_snapshot(
            &self.settings,
            &self.timer,
            self.edit_mode,
            now,
        ));
    }

    fn show_control_window(&mut self, root_ui: &mut egui::Ui, now: Instant) {
        let palette = Palette::for_mode(self.settings.dark_mode);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.canvas))
            .show(root_ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .content_margin(egui::Margin::symmetric(22, 18))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        self.show_header(ui, palette);
                        ui.add_space(14.0);
                        self.show_timer_console(ui, now, palette);
                        ui.add_space(14.0);
                        self.show_output_panel(ui, palette);
                        ui.add_space(10.0);
                        self.show_hotkey_help(ui, palette);
                    });
            });
    }

    fn show_header(&mut self, ui: &mut egui::Ui, palette: Palette) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Overlay Timer")
                        .size(24.0)
                        .color(palette.text)
                        .strong(),
                );
                ui.label(
                    RichText::new("Presenter-Konsole")
                        .size(13.0)
                        .color(palette.muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                let tooltip = if self.settings.dark_mode {
                    "Helles Design"
                } else {
                    "Dunkles Design"
                };
                if theme_toggle_button(ui, self.settings.dark_mode, palette)
                    .on_hover_text(tooltip)
                    .clicked()
                {
                    self.settings.dark_mode = !self.settings.dark_mode;
                    configure_theme(ui.ctx(), self.settings.dark_mode);
                    ui.ctx().request_repaint();
                }
            });
        });
    }

    fn show_timer_console(&mut self, ui: &mut egui::Ui, now: Instant, palette: Palette) {
        let reading = self.timer.reading(now);
        surface(ui, palette, 16, |ui| {
            ui.vertical_centered(|ui| {
                let editable = self.timer.state() == TimerState::Paused && !reading.is_overtime();
                if editable {
                    let mut minutes = self.settings.duration_seconds / 60;
                    let mut seconds = self.settings.duration_seconds % 60;
                    if duration_editor(ui, &mut minutes, &mut seconds, palette) {
                        self.set_duration(minutes * 60 + seconds);
                    }
                } else {
                    let time_color = if reading.is_overtime() {
                        palette.error
                    } else {
                        palette.text
                    };
                    ui.label(
                        RichText::new(format_reading(reading))
                            .size(54.0)
                            .color(time_color)
                            .strong()
                            .monospace(),
                    );
                }
                status_chip(ui, self.timer.state(), reading, palette);
            });

            ui.add_space(12.0);
            ui.add_enabled_ui(!self.timer.is_running(), |ui| {
                let presets = [
                    (60, "1 min"),
                    (300, "5 min"),
                    (600, "10 min"),
                    (900, "15 min"),
                ];
                let button_width = ((ui.available_width() - 24.0) / 4.0).max(62.0);
                ui.horizontal(|ui| {
                    for (seconds, label) in presets {
                        let selected =
                            self.settings.duration_seconds == seconds && !reading.is_overtime();
                        if ui
                            .add_sized(
                                [button_width, 32.0],
                                egui::Button::new(label).selected(selected),
                            )
                            .clicked()
                        {
                            self.set_duration(seconds);
                        }
                    }
                });
            });

            ui.add_space(6.0);
            let reset_width = 86.0;
            let primary_width = (ui.available_width() - reset_width - 10.0).max(160.0);
            ui.horizontal(|ui| {
                let start_label = match (self.timer.state(), reading.is_overtime()) {
                    (TimerState::Running, _) => "Pause",
                    (TimerState::Paused, true) => "Fortsetzen",
                    (TimerState::Paused, false) => "Start",
                };
                if ui
                    .add_enabled(
                        self.settings.duration_seconds > 0 || reading.is_overtime(),
                        primary_button(start_label, primary_width, palette),
                    )
                    .clicked()
                {
                    self.toggle_timer(now);
                }
                if ui
                    .add(secondary_button("Reset", reset_width, palette))
                    .clicked()
                {
                    self.timer.reset();
                }
            });
        });
    }

    fn show_output_panel(&mut self, ui: &mut egui::Ui, palette: Palette) {
        surface(ui, palette, 14, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Ausgabe")
                            .size(17.0)
                            .color(palette.text)
                            .strong(),
                    );
                    ui.label(
                        RichText::new(if self.settings.overlay_visible {
                            "Overlay ist sichtbar"
                        } else {
                            "Overlay ist ausgeblendet"
                        })
                        .size(12.0)
                        .color(palette.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if toggle_switch(ui, &mut self.settings.overlay_visible, palette).changed()
                        && !self.settings.overlay_visible
                    {
                        self.edit_mode = false;
                    }
                });
            });

            ui.add_space(12.0);
            ui.label(
                RichText::new("Zielbildschirm")
                    .size(12.0)
                    .color(palette.muted),
            );
            ui.horizontal(|ui| {
                let selected = self.monitors.get(self.settings.monitor_index).map_or_else(
                    || "Unbekannter Bildschirm".to_owned(),
                    |monitor| monitor.label(self.settings.monitor_index),
                );
                let combo_width = (ui.available_width() - 46.0).max(180.0);
                egui::ComboBox::from_id_salt("monitor_selector")
                    .width(combo_width)
                    .truncate()
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for (index, monitor) in self.monitors.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.settings.monitor_index,
                                index,
                                monitor.label(index),
                            );
                        }
                    });
                if ui
                    .add_sized(
                        [38.0, 38.0],
                        egui::Button::new(RichText::new("↻").size(17.0)),
                    )
                    .on_hover_text("Bildschirme neu einlesen")
                    .clicked()
                {
                    self.monitors = monitors::enumerate();
                    self.clamp_monitor_index();
                }
            });

            ui.add_space(10.0);
            ui.label(RichText::new("Position").size(12.0).color(palette.muted));
            position_selector(
                ui,
                &mut self.settings.position,
                &mut self.settings.custom_position,
                palette,
            );

            let custom_active = self.settings.custom_position.is_some() || self.edit_mode;
            let edit_text = if self.edit_mode {
                "Bearbeitung beenden"
            } else {
                "Frei positionieren"
            };
            let edit_button = egui::Button::new(
                RichText::new(edit_text)
                    .color(if custom_active {
                        Color32::WHITE
                    } else {
                        palette.primary
                    })
                    .strong(),
            )
            .fill(if custom_active {
                palette.primary_fill
            } else {
                palette.surface
            })
            .stroke(egui::Stroke::new(1.0, palette.primary))
            .corner_radius(9.0);
            let can_edit = self.settings.overlay_visible && !self.timer.is_running();
            let edit_button_width = (ui.available_width() - 96.0).max(160.0);
            let edit_clicked = ui
                .horizontal(|ui| {
                    ui.add_space(((ui.available_width() - edit_button_width) / 2.0).max(0.0));
                    ui.add_enabled_ui(can_edit, |ui| {
                        ui.add_sized([edit_button_width, 38.0], edit_button)
                    })
                    .inner
                    .clicked()
                })
                .inner;
            if edit_clicked {
                self.edit_mode = !self.edit_mode;
                ui.ctx().send_viewport_cmd_to(
                    OverlayBridge::viewport_id(),
                    egui::ViewportCommand::MousePassthrough(!self.edit_mode),
                );
            }
            if self.edit_mode {
                helper_text(
                    ui,
                    "Timerkarte zum Verschieben ziehen; Eckgriffe skalieren.",
                    palette,
                );
            }

            ui.add_space(4.0);
            egui::CollapsingHeader::new(RichText::new("Darstellung").color(palette.text).strong())
                .id_salt("appearance_settings")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    slider_row(
                        ui,
                        "Randabstand",
                        format!("{:.0} px", self.settings.margin),
                        palette,
                        self.settings.custom_position.is_none(),
                        |ui| {
                            ui.add(
                                egui::Slider::new(&mut self.settings.margin, 0.0..=160.0)
                                    .show_value(false)
                                    .trailing_fill(true),
                            );
                        },
                    );
                    slider_row(
                        ui,
                        "Schriftgröße",
                        format!("{:.0} pt", self.settings.font_size),
                        palette,
                        true,
                        |ui| {
                            ui.add(
                                egui::Slider::new(
                                    &mut self.settings.font_size,
                                    MIN_FONT_SIZE..=MAX_FONT_SIZE,
                                )
                                .show_value(false)
                                .trailing_fill(true),
                            );
                        },
                    );
                    let opacity = u16::from(self.settings.background_opacity) * 100 / 255;
                    slider_row(
                        ui,
                        "Hintergrund",
                        format!("{opacity} %"),
                        palette,
                        true,
                        |ui| {
                            ui.add(
                                egui::Slider::new(&mut self.settings.background_opacity, 0..=240)
                                    .show_value(false)
                                    .trailing_fill(true),
                            );
                        },
                    );
                });
        });
    }

    fn show_hotkey_help(&self, ui: &mut egui::Ui, palette: Palette) {
        egui::CollapsingHeader::new(
            RichText::new("Tastenkürzel anzeigen")
                .size(13.0)
                .color(palette.muted),
        )
        .id_salt("hotkey_help")
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("hotkey_grid")
                .num_columns(2)
                .spacing(egui::vec2(16.0, 8.0))
                .show(ui, |ui| {
                    keycap(ui, "Ctrl+Alt+P", palette);
                    ui.label(RichText::new("Start/Pause").color(palette.muted));
                    ui.end_row();
                    keycap(ui, "Ctrl+Alt+R", palette);
                    ui.label(RichText::new("Reset").color(palette.muted));
                    ui.end_row();
                    keycap(ui, "Ctrl+Alt+O", palette);
                    ui.label(RichText::new("Overlay ein/aus").color(palette.muted));
                    ui.end_row();
                });
            if let Some(error) = &self.hotkeys.error {
                error_box(
                    ui,
                    &format!("Hotkey-Registrierung unvollständig: {error}"),
                    palette,
                );
            }
            if let Some(error) = &self.tray_error {
                error_box(
                    ui,
                    &format!("System-Tray nicht verfügbar: {error}"),
                    palette,
                );
            }
        });
    }
}

fn theme_toggle_button(ui: &mut egui::Ui, dark_mode: bool, palette: Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), egui::Sense::click());
    let painter = ui.painter();

    if response.hovered() {
        painter.rect_filled(rect, 10.0, palette.surface_high);
    }

    let center = rect.center();
    if dark_mode {
        painter.circle_filled(center, 4.5, palette.text);
        for step in 0..8 {
            let angle = step as f32 * std::f32::consts::TAU / 8.0;
            let direction = egui::vec2(angle.cos(), angle.sin());
            painter.line_segment(
                [center + direction * 7.5, center + direction * 10.5],
                egui::Stroke::new(1.5, palette.text),
            );
        }
    } else {
        painter.circle_filled(center, 8.5, palette.text);
        painter.circle_filled(center + egui::vec2(4.0, -3.0), 8.0, palette.canvas);
    }

    response
}

impl eframe::App for OverlayTimerApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.settings);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.theme_initialized_after_startup {
            configure_theme(ctx, self.settings.dark_mode);
            self.theme_initialized_after_startup = true;
        }
        let now = Instant::now();
        let overlay_was_visible = self.settings.overlay_visible;
        self.handle_minimize(ctx);
        self.process_overlay_events();
        self.process_hotkeys(now);
        self.process_tray(ctx, now);
        self.sync_tray(now);
        self.sync_overlay(now);
        if overlay_was_visible != self.settings.overlay_visible {
            OverlayBridge::set_visible(ctx, self.settings.overlay_visible);
        }
        OverlayBridge::request_repaint(ctx);
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        self.show_control_window(ui, now);
        self.sync_tray(now);
        self.sync_overlay(now);
        self.overlay.show(ui);
    }
}

fn overlay_snapshot(
    settings: &Settings,
    timer: &CountdownTimer,
    edit_mode: bool,
    now: Instant,
) -> OverlaySnapshot {
    let reading = timer.reading(now);
    let (anchor, anchor_offset) = settings.position.anchor(settings.margin);
    OverlaySnapshot {
        visible: settings.overlay_visible,
        monitor_index: settings.monitor_index,
        anchor,
        anchor_offset,
        custom_position: settings.custom_position,
        font_size: settings.font_size,
        text_color: settings.text_color,
        background_opacity: settings.background_opacity,
        edit_mode,
        time: format_reading(reading),
        overtime: reading.is_overtime(),
    }
}

fn configure_theme(ctx: &egui::Context, dark_mode: bool) {
    let palette = Palette::for_mode(dark_mode);
    let mut visuals = if dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.canvas;
    visuals.window_fill = palette.surface;
    visuals.window_stroke = egui::Stroke::new(1.0, palette.outline);
    visuals.selection.bg_fill = palette.primary_fill;
    visuals.selection.stroke.color = Color32::WHITE;
    visuals.override_text_color = Some(palette.text);
    visuals.weak_text_color = Some(palette.muted);
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = 8.0.into();
    }
    visuals.widgets.inactive.bg_fill = palette.surface_high;
    visuals.widgets.inactive.weak_bg_fill = palette.surface_high;
    visuals.widgets.hovered.bg_fill = palette.outline;
    visuals.widgets.active.bg_fill = palette.primary_fill;
    visuals.widgets.open.bg_fill = palette.surface_high;
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.interact_size.y = 36.0;
        style.spacing.slider_width = 320.0;
    });
}

fn surface<R>(
    ui: &mut egui::Ui,
    palette: Palette,
    radius: u8,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(palette.surface)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(radius)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

fn duration_editor(
    ui: &mut egui::Ui,
    minutes: &mut u64,
    seconds: &mut u64,
    palette: Palette,
) -> bool {
    let mut changed = false;
    let total_width = 236.0;
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - total_width) / 2.0).max(0.0));
        for (index, value) in [minutes, seconds].into_iter().enumerate() {
            if index == 1 {
                ui.label(
                    RichText::new(":")
                        .size(46.0)
                        .color(palette.text)
                        .strong()
                        .monospace(),
                );
            }
            egui::Frame::new()
                .fill(palette.surface_high)
                .stroke(egui::Stroke::new(1.0, palette.outline))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.style_mut().override_font_id = Some(egui::FontId::monospace(42.0));
                    let range = if index == 0 { 0..=599 } else { 0..=59 };
                    changed |= ui
                        .add_sized(
                            [82.0, 58.0],
                            egui::DragValue::new(value)
                                .range(range)
                                .speed(1.0)
                                .custom_formatter(|number, _| {
                                    format!("{:02}", number.round() as u64)
                                })
                                .custom_parser(|text| {
                                    text.trim().parse::<u64>().ok().map(|v| v as f64)
                                }),
                        )
                        .on_hover_text("Klicken oder ziehen, um die Zeit zu ändern")
                        .changed();
                });
        }
    });
    changed
}

fn primary_button(text: &str, width: f32, palette: Palette) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(palette.primary_fill)
        .stroke(egui::Stroke::NONE)
        .corner_radius(9.0)
        .min_size(egui::vec2(width, 42.0))
}

fn secondary_button(text: &str, width: f32, palette: Palette) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(palette.primary).strong())
        .fill(palette.surface)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(9.0)
        .min_size(egui::vec2(width, 42.0))
}

fn status_chip(ui: &mut egui::Ui, state: TimerState, reading: TimerReading, palette: Palette) {
    let (label, text_color, fill_color) = if reading.is_overtime() {
        let label = if state == TimerState::Paused {
            "Überzogen · pausiert"
        } else {
            "Überzogen"
        };
        (label, palette.error, palette.error_bg)
    } else if state == TimerState::Running {
        ("Läuft", palette.success, palette.success_bg)
    } else {
        ("Pausiert", palette.paused, palette.paused_bg)
    };
    egui::Frame::new()
        .fill(fill_color)
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(12.0).color(text_color).strong());
        });
}

fn toggle_switch(ui: &mut egui::Ui, enabled: &mut bool, palette: Palette) -> egui::Response {
    let desired_size = egui::vec2(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *enabled = !*enabled;
        response.mark_changed();
    }
    let progress = ui.ctx().animate_bool(response.id, *enabled);
    let track = if *enabled {
        palette.primary_fill
    } else {
        palette.surface_high
    };
    ui.painter().rect_filled(rect, 12.0, track);
    ui.painter().rect_stroke(
        rect,
        12.0,
        egui::Stroke::new(1.0, palette.outline),
        egui::StrokeKind::Inside,
    );
    let knob_x = egui::lerp((rect.left() + 12.0)..=(rect.right() - 12.0), progress);
    ui.painter()
        .circle_filled(egui::pos2(knob_x, rect.center().y), 8.5, Color32::WHITE);
    response
}

fn position_selector(
    ui: &mut egui::Ui,
    position: &mut OverlayPosition,
    custom_position: &mut Option<[f32; 2]>,
    palette: Palette,
) {
    let desired_width = ui.available_width().min(300.0);
    ui.vertical_centered(|ui| {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(desired_width, 124.0), egui::Sense::hover());
        let preview = rect.shrink2(egui::vec2(12.0, 8.0));
        ui.painter()
            .rect_filled(preview, 10.0, palette.surface_high);
        ui.painter().rect_stroke(
            preview,
            10.0,
            egui::Stroke::new(1.0, palette.outline),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            preview.center(),
            Align2::CENTER_CENTER,
            "Overlay-Position",
            egui::FontId::proportional(11.0),
            palette.muted,
        );

        for corner in OverlayPosition::ALL {
            let center = match corner {
                OverlayPosition::TopLeft => preview.left_top() + egui::vec2(30.0, 22.0),
                OverlayPosition::TopRight => preview.right_top() + egui::vec2(-30.0, 22.0),
                OverlayPosition::BottomLeft => preview.left_bottom() + egui::vec2(30.0, -22.0),
                OverlayPosition::BottomRight => preview.right_bottom() + egui::vec2(-30.0, -22.0),
            };
            let hit_rect = egui::Rect::from_center_size(center, egui::vec2(48.0, 30.0));
            let response = ui.interact(hit_rect, ui.id().with(corner), egui::Sense::click());
            let selected = custom_position.is_none() && *position == corner;
            if response.clicked() {
                *position = corner;
                *custom_position = None;
            }
            let fill = if selected {
                palette.primary_fill
            } else if response.hovered() {
                palette.outline
            } else {
                palette.surface
            };
            ui.painter().rect_filled(hit_rect, 7.0, fill);
            ui.painter().rect_stroke(
                hit_rect,
                7.0,
                egui::Stroke::new(
                    1.0,
                    if selected {
                        palette.primary
                    } else {
                        palette.outline
                    },
                ),
                egui::StrokeKind::Inside,
            );
            let marker = egui::Rect::from_center_size(center, egui::vec2(24.0, 8.0));
            ui.painter().rect_filled(
                marker,
                3.0,
                if selected {
                    Color32::WHITE
                } else {
                    palette.muted
                },
            );
            response.on_hover_text(corner.label());
        }
    });
    if custom_position.is_none() {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(position.label())
                    .size(12.0)
                    .color(palette.muted),
            );
        });
    }
}

fn slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: String,
    palette: Palette,
    enabled: bool,
    add_slider: impl FnOnce(&mut egui::Ui),
) {
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).color(palette.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(value).color(palette.text).monospace());
            });
        });
        ui.scope(|ui| {
            ui.spacing_mut().slider_width = ui.available_width();
            add_slider(ui);
        });
    });
    ui.add_space(5.0);
}

fn helper_text(ui: &mut egui::Ui, text: &str, palette: Palette) {
    ui.label(RichText::new(text).size(12.0).color(palette.muted));
}

fn keycap(ui: &mut egui::Ui, text: &str, palette: Palette) {
    egui::Frame::new()
        .fill(palette.surface_high)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(palette.text).monospace());
        });
}

fn error_box(ui: &mut egui::Ui, text: &str, palette: Palette) {
    ui.add_space(6.0);
    egui::Frame::new()
        .fill(palette.error_bg)
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(palette.error));
        });
}
