use std::sync::Arc;

use eframe::egui;

const ICON_SIZE: u32 = 32;

pub fn app_icon() -> Arc<egui::IconData> {
    Arc::new(egui::IconData {
        rgba: icon_rgba(),
        width: ICON_SIZE,
        height: ICON_SIZE,
    })
}

pub fn tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    tray_icon::Icon::from_rgba(icon_rgba(), ICON_SIZE, ICON_SIZE)
}

fn icon_rgba() -> Vec<u8> {
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    let center = (ICON_SIZE as f32 - 1.0) * 0.5;

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let in_face = distance <= 13.5;
            let in_ring = (10.5..=13.5).contains(&distance);
            let minute_hand = dx.abs() <= 1.25 && (-7.0..=0.5).contains(&dy);
            let hour_hand = (-0.5..=6.5).contains(&dx) && dy.abs() <= 1.25;

            let pixel = if in_ring || (in_face && (minute_hand || hour_hand)) {
                [255, 255, 255, 255]
            } else if in_face {
                [67, 85, 185, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba.extend_from_slice(&pixel);
        }
    }

    rgba
}
