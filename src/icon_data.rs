pub const ICON_SIZE: u32 = 32;

pub fn icon_rgba(size: u32) -> Vec<u8> {
    assert!(size > 0);

    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let center = (size as f32 - 1.0) * 0.5;
    let scale = size as f32 / ICON_SIZE as f32;
    let outer_radius = 13.5 * scale;
    let inner_radius = 10.5 * scale;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let in_face = distance <= outer_radius;
            let in_ring = (inner_radius..=outer_radius).contains(&distance);
            let minute_hand =
                dx.abs() <= 1.25 * scale && (-7.0 * scale..=0.5 * scale).contains(&dy);
            let hour_hand = (-0.5 * scale..=6.5 * scale).contains(&dx) && dy.abs() <= 1.25 * scale;

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

#[cfg(test)]
mod tests {
    use super::{ICON_SIZE, icon_rgba};

    #[test]
    fn runtime_icon_has_the_expected_dimensions() {
        assert_eq!(
            icon_rgba(ICON_SIZE).len(),
            (ICON_SIZE * ICON_SIZE * 4) as usize
        );
    }
}
