#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icon;
mod monitors;
mod overlay;
mod timer;
mod tray;
mod windows_overlay;

use app::OverlayTimerApp;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Overlay Timer – Steuerung")
            .with_app_id("overlay-timer")
            .with_icon(icon::app_icon())
            .with_inner_size([520.0, 760.0])
            .with_min_inner_size([440.0, 620.0]),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "Overlay Timer",
        options,
        Box::new(|creation_context| Ok(Box::new(OverlayTimerApp::new(creation_context)))),
    )
}
