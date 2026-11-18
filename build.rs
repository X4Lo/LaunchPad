// The same rasteriser the app uses at runtime, so exe / taskbar / tray icons always match.
#[allow(dead_code)]
#[path = "src/ui/icon.rs"]
mod icon;

use std::path::Path;

fn main() {
    // Same widget style on every OS so the app looks identical everywhere.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/main.slint", config).expect("Slint UI failed to compile");

    println!("cargo:rerun-if-changed=src/ui/icon.rs");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" {
        let out = std::env::var("OUT_DIR").unwrap();
        let images = icon_images();
        std::fs::write(Path::new(&out).join("launchpad.ico"), ico_file(&images)).unwrap();
        if target_env == "msvc" {
            // link.exe accepts compiled resource files directly: no rc.exe / Windows SDK needed.
            let res = Path::new(&out).join("launchpad.res");
            std::fs::write(&res, res_file(&images)).unwrap();
            println!("cargo:rustc-link-arg-bins={}", res.display());
        }
    }
}

const SIZES: [u32; 7] = [16, 20, 24, 32, 48, 64, 256];

/// Each size as an ICO-style DIB: BITMAPINFOHEADER + bottom-up BGRA + 1-bit AND mask.
fn icon_images() -> Vec<(u32, Vec<u8>)> {
    SIZES
        .iter()
        .map(|&size| {
            let rgba = icon::render(size);
            let mut dib = Vec::new();
            for v in [40u32, size, size * 2] {
                dib.extend_from_slice(&v.to_le_bytes()); // biSize, biWidth, biHeight (xor+and)
            }
            dib.extend_from_slice(&1u16.to_le_bytes()); // planes
            dib.extend_from_slice(&32u16.to_le_bytes()); // bit count
            dib.extend_from_slice(&[0u8; 24]); // compression .. clrImportant
            for y in (0..size).rev() {
                for x in 0..size {
                    let i = ((y * size + x) * 4) as usize;
                    dib.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
                }
            }
            let mask_row = size.div_ceil(32) * 4; // rows padded to 32 bits; all zero = use alpha
            dib.extend(std::iter::repeat_n(0u8, (mask_row * size) as usize));
            (size, dib)
        })
        .collect()
}

fn dim(size: u32) -> u8 {
    if size >= 256 {
        0
    } else {
        size as u8
    }
}

fn ico_file(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend_from_slice(&[0, 0, 1, 0]);
    f.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, dib) in images {
        f.extend_from_slice(&[dim(*size), dim(*size), 0, 0]);
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&32u16.to_le_bytes());
        f.extend_from_slice(&(dib.len() as u32).to_le_bytes());
        f.extend_from_slice(&offset.to_le_bytes());
        offset += dib.len() as u32;
    }
    for (_, dib) in images {
        f.extend_from_slice(dib);
    }
    f
}

/// A compiled Win32 resource file (.res) with RT_ICON entries and RT_GROUP_ICON #1
/// (the lowest-numbered group icon is what Explorer shows for the exe).
fn res_file(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    const LANG_EN_US: u16 = 0x0409;
    let mut f = Vec::new();
    res_entry(&mut f, 0, 0, 0, &[], true); // mandatory empty first entry
    for (i, (_, dib)) in images.iter().enumerate() {
        res_entry(&mut f, RT_ICON, i as u16 + 1, LANG_EN_US, dib, false);
    }
    let mut group = Vec::new();
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&(images.len() as u16).to_le_bytes());
    for (i, (size, dib)) in images.iter().enumerate() {
        group.extend_from_slice(&[dim(*size), dim(*size), 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&(dib.len() as u32).to_le_bytes());
        group.extend_from_slice(&(i as u16 + 1).to_le_bytes());
    }
    res_entry(&mut f, RT_GROUP_ICON, 1, LANG_EN_US, &group, false);
    f
}

fn res_entry(f: &mut Vec<u8>, kind: u16, id: u16, lang: u16, data: &[u8], empty: bool) {
    f.extend_from_slice(&(data.len() as u32).to_le_bytes()); // DataSize
    f.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize
    if empty {
        f.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
    } else {
        f.extend_from_slice(&0xFFFFu16.to_le_bytes());
        f.extend_from_slice(&kind.to_le_bytes());
        f.extend_from_slice(&0xFFFFu16.to_le_bytes());
        f.extend_from_slice(&id.to_le_bytes());
    }
    f.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    let flags: u16 = if empty { 0 } else { 0x1010 }; // MOVEABLE | DISCARDABLE
    f.extend_from_slice(&flags.to_le_bytes());
    f.extend_from_slice(&lang.to_le_bytes());
    f.extend_from_slice(&0u32.to_le_bytes()); // Version
    f.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    f.extend_from_slice(data);
    while f.len() % 4 != 0 {
        f.push(0);
    }
}
