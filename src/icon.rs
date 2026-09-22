use std::sync::Arc;

use eframe::egui;

use crate::icon_data::{ICON_SIZE, icon_rgba};

pub fn app_icon() -> Arc<egui::IconData> {
    Arc::new(egui::IconData {
        rgba: icon_rgba(ICON_SIZE),
        width: ICON_SIZE,
        height: ICON_SIZE,
    })
}

pub fn tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    tray_icon::Icon::from_rgba(icon_rgba(ICON_SIZE), ICON_SIZE, ICON_SIZE)
}
