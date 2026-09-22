#[path = "src/icon_data.rs"]
mod icon_data;

#[cfg(windows)]
fn main() {
    use std::{env, fs, path::PathBuf};

    const ICON_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let icon_path = output_dir.join("overlay-timer.ico");
    fs::write(&icon_path, build_ico(ICON_SIZES)).expect("write generated application icon");

    let resource_path = output_dir.join("overlay-timer.rc");
    let escaped_icon_path = icon_path.display().to_string().replace('\\', "\\\\");
    fs::write(&resource_path, format!("1 ICON \"{escaped_icon_path}\"\n"))
        .expect("write Windows resource definition");

    println!("cargo:rerun-if-changed=src/icon_data.rs");
    embed_resource::compile_for(&resource_path, ["overlay-timer"], embed_resource::NONE)
        .manifest_optional()
        .expect("compile Windows application icon");
}

#[cfg(not(windows))]
fn main() {
    println!("cargo:rerun-if-changed=src/icon_data.rs");
}

#[cfg(windows)]
fn build_ico(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().copied().map(icon_bitmap).collect();
    let directory_size = 6 + sizes.len() * 16;
    let mut offset = directory_size as u32;
    let mut ico = Vec::with_capacity(directory_size + images.iter().map(Vec::len).sum::<usize>());

    push_u16(&mut ico, 0);
    push_u16(&mut ico, 1);
    push_u16(&mut ico, sizes.len() as u16);

    for (&size, image) in sizes.iter().zip(&images) {
        assert!((1..=256).contains(&size));
        ico.push(if size == 256 { 0 } else { size as u8 });
        ico.push(if size == 256 { 0 } else { size as u8 });
        ico.push(0);
        ico.push(0);
        push_u16(&mut ico, 1);
        push_u16(&mut ico, 32);
        push_u32(&mut ico, image.len() as u32);
        push_u32(&mut ico, offset);
        offset += image.len() as u32;
    }

    for image in images {
        ico.extend_from_slice(&image);
    }

    ico
}

#[cfg(windows)]
fn icon_bitmap(size: u32) -> Vec<u8> {
    let rgba = icon_data::icon_rgba(size);
    let mask_stride = size.div_ceil(32) * 4;
    let color_bytes = size * size * 4;
    let mut bitmap = Vec::with_capacity((40 + color_bytes + mask_stride * size) as usize);

    push_u32(&mut bitmap, 40);
    push_i32(&mut bitmap, size as i32);
    push_i32(&mut bitmap, (size * 2) as i32);
    push_u16(&mut bitmap, 1);
    push_u16(&mut bitmap, 32);
    push_u32(&mut bitmap, 0);
    push_u32(&mut bitmap, color_bytes);
    push_i32(&mut bitmap, 0);
    push_i32(&mut bitmap, 0);
    push_u32(&mut bitmap, 0);
    push_u32(&mut bitmap, 0);

    for y in (0..size).rev() {
        for x in 0..size {
            let index = ((y * size + x) * 4) as usize;
            bitmap.extend_from_slice(&[
                rgba[index + 2],
                rgba[index + 1],
                rgba[index],
                rgba[index + 3],
            ]);
        }
    }

    for y in (0..size).rev() {
        let mut mask_row = vec![0_u8; mask_stride as usize];
        for x in 0..size {
            let alpha_index = ((y * size + x) * 4 + 3) as usize;
            if rgba[alpha_index] == 0 {
                mask_row[(x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
        bitmap.extend_from_slice(&mask_row);
    }

    bitmap
}

#[cfg(windows)]
fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(windows)]
fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(windows)]
fn push_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_le_bytes());
}
