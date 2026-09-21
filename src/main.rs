#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icon;
mod meme;
mod monitors;
mod overlay;
mod timer;
mod tray;
mod windows_overlay;

use app::OverlayTimerApp;
use eframe::egui;

fn main() -> eframe::Result {
    // Vulkan crashes in some Intel drivers, while DX12 can reject the transparent
    // fullscreen overlay surface. wgpu's GL backend works for both viewports.
    let mut wgpu_options = eframe::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::GL;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Overlay Timer – Steuerung")
            .with_app_id("overlay-timer")
            .with_icon(icon::app_icon())
            .with_inner_size([520.0, 760.0])
            .with_min_inner_size([440.0, 620.0]),
        persist_window: true,
        wgpu_options,
        ..Default::default()
    };

    eframe::run_native(
        "Overlay Timer",
        options,
        Box::new(|creation_context| Ok(Box::new(OverlayTimerApp::new(creation_context)))),
    )
}
