use egui::{Color32, TextureHandle};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::models::item::LaunchItem;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconKey {
    Custom(PathBuf, u32),
    DefaultApp(PathBuf, u32), // executable path + size — for extraction
    DefaultGroup,
    DefaultFolder,
}

pub struct IconCache {
    textures: HashMap<IconKey, TextureHandle>,
    icons_dir: PathBuf,
}

impl IconCache {
    pub fn new(icons_dir: PathBuf) -> Self {
        Self {
            textures: HashMap::new(),
            icons_dir,
        }
    }

    /// Resolve an icon path: if it's just a filename, prepend the icons directory.
    fn resolve_path(&self, path: &PathBuf) -> PathBuf {
        // If the path has no directory separators, it's a bare filename
        if path.parent().map_or(true, |p| p.as_os_str().is_empty()) {
            self.icons_dir.join(path)
        } else {
            path.clone()
        }
    }

    pub fn get_or_load(&mut self, key: IconKey, ctx: &egui::Context) -> Option<&TextureHandle> {
        if self.textures.contains_key(&key) {
            return self.textures.get(&key);
        }

        let texture = match &key {
            IconKey::Custom(path, size) => {
                let resolved = self.resolve_path(path);
                load_icon_from_file(&resolved, *size, ctx)
            }
            IconKey::DefaultApp(exe_path, size) => {
                // Try to extract the real icon from the executable
                extract_and_load(exe_path, *size, ctx)
                    .or_else(|| Some(generate_default_app_icon(ctx)))
            }
            IconKey::DefaultGroup => Some(generate_default_group_icon(ctx)),
            IconKey::DefaultFolder => Some(generate_default_folder_icon(ctx)),
        };

        if let Some(tex) = texture {
            self.textures.insert(key.clone(), tex);
        }
        self.textures.get(&key)
    }

    pub fn key_for(item: &LaunchItem, icon_size: u32) -> IconKey {
        match item {
            LaunchItem::App(app) => {
                if let Some(ref path) = app.icon_path {
                    IconKey::Custom(path.clone(), icon_size)
                } else {
                    IconKey::DefaultApp(app.executable_path.clone(), icon_size)
                }
            }
            LaunchItem::Group(group) => {
                if let Some(ref path) = group.icon_path {
                    IconKey::Custom(path.clone(), icon_size)
                } else {
                    IconKey::DefaultGroup
                }
            }
            LaunchItem::Folder(folder) => {
                if let Some(ref path) = folder.icon_path {
                    IconKey::Custom(path.clone(), icon_size)
                } else {
                    IconKey::DefaultFolder
                }
            }
        }
    }
}

// ─── Icon extraction (Windows) ───────────────────────────

#[cfg(windows)]
fn extract_and_load(exe_path: &PathBuf, size: u32, ctx: &egui::Context) -> Option<TextureHandle> {
    let rgba = extract_icon_rgba(exe_path, size)?;
    let color_image = egui::ColorImage::from_rgba_unmultiplied([size as _, size as _], &rgba);
    let label = format!("exe_icon_{}", exe_path.display());
    Some(ctx.load_texture(label, color_image, egui::TextureOptions::LINEAR))
}

#[cfg(not(windows))]
fn extract_and_load(
    _exe_path: &PathBuf,
    _size: u32,
    _ctx: &egui::Context,
) -> Option<TextureHandle> {
    None
}

#[cfg(windows)]
fn extract_icon_rgba(path: &PathBuf, size: u32) -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS, RGBQUAD,
    };
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL};

    let path_str = path.to_string_lossy();
    let wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        // Try to extract the real icon from the file first (requires file to exist)
        let mut info = SHFILEINFOW::default();
        let mut result = SHGetFileInfoW(
            PCWSTR::from_raw(wide.as_ptr()),
            Default::default(), // dwFileAttributes: 0 for real files
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON, // no USEFILEATTRIBUTES — try real file
        );
        // If the file doesn't exist, fall back to USEFILEATTRIBUTES
        if result == 0 || info.hIcon.is_invalid() {
            info = SHFILEINFOW::default();
            result = SHGetFileInfoW(
                PCWSTR::from_raw(wide.as_ptr()),
                FILE_ATTRIBUTE_NORMAL,
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
            );
        }
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }

        let w = size as i32;
        let h = size as i32;

        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            let _ = DestroyIcon(info.hIcon);
            return None;
        }

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default(); 1],
        };

        let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0) {
            Ok(b) if !b.is_invalid() && !bits_ptr.is_null() => b,
            _ => {
                let _ = DeleteDC(hdc);
                let _ = DestroyIcon(info.hIcon);
                return None;
            }
        };

        let old_bmp = SelectObject(hdc, bitmap);

        // Zero the buffer so uncovered pixels are transparent
        let byte_count = (w * h * 4) as usize;
        std::ptr::write_bytes(bits_ptr as *mut u8, 0, byte_count);

        // Draw the icon scaled to our size
        let _ = DrawIconEx(hdc, 0, 0, info.hIcon, w, h, 0, None, DI_NORMAL);

        // Read pixels directly from the DIB section's buffer — no extra copy needed
        let pixels = std::slice::from_raw_parts(bits_ptr as *const u8, byte_count).to_vec();

        SelectObject(hdc, old_bmp);
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(hdc);
        let _ = DestroyIcon(info.hIcon);

        // BGRA → RGBA
        let mut pixels = pixels;
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2); // swap B and R
        }

        Some(pixels)
    }
}

// ─── Default icon generation (fallback) ──────────────────

fn generate_default_app_icon(ctx: &egui::Context) -> TextureHandle {
    let size = 64i32;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let bg = Color32::from_rgb(60, 60, 80);
    let accent = Color32::from_rgb(137, 180, 250);
    let radius = size / 8;

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let in_corner = |cx: i32, cy: i32| -> bool {
                (x - cx) * (x - cx) + (y - cy) * (y - cy) > radius * radius
            };
            if (x < radius && y < radius && in_corner(radius, radius))
                || (x >= size - radius && y < radius && in_corner(size - radius - 1, radius))
                || (x < radius && y >= size - radius && in_corner(radius, size - radius - 1))
                || (x >= size - radius
                    && y >= size - radius
                    && in_corner(size - radius - 1, size - radius - 1))
            {
                pixels[idx + 3] = 0;
                continue;
            }
            pixels[idx] = bg.r();
            pixels[idx + 1] = bg.g();
            pixels[idx + 2] = bg.b();
            pixels[idx + 3] = bg.a();
        }
    }
    let inset = 14;
    for y in inset..size - inset {
        for x in inset..size - inset {
            let idx = ((y * size + x) * 4) as usize;
            let b = 2;
            if x < inset + b || x >= size - inset - b || y < inset + b || y >= size - inset - b {
                pixels[idx] = accent.r();
                pixels[idx + 1] = accent.g();
                pixels[idx + 2] = accent.b();
                pixels[idx + 3] = accent.a();
            }
        }
    }
    let ci = egui::ColorImage::from_rgba_unmultiplied([size as _, size as _], &pixels);
    ctx.load_texture("default_app_icon", ci, egui::TextureOptions::LINEAR)
}

fn generate_default_group_icon(ctx: &egui::Context) -> TextureHandle {
    let size = 64i32;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let fc = Color32::from_rgb(242, 201, 76);
    let tc = Color32::from_rgb(255, 224, 130);
    let radius = size / 8;
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let ic = |cx: i32, cy: i32| -> bool {
                (x - cx) * (x - cx) + (y - cy) * (y - cy) > radius * radius
            };
            let th = 10;
            let tw = 22;
            if y < th && x < tw {
                if (x < radius && y < radius && ic(radius, radius))
                    || (x >= tw - radius && y < radius && ic(tw - radius - 1, radius))
                {
                    pixels[idx + 3] = 0;
                    continue;
                }
                pixels[idx] = tc.r();
                pixels[idx + 1] = tc.g();
                pixels[idx + 2] = tc.b();
                pixels[idx + 3] = 255;
            } else if y >= 6 {
                if (x < radius && y < 6 + radius && ic(radius, 6 + radius))
                    || (x >= size - radius && y < 6 + radius && ic(size - radius - 1, 6 + radius))
                    || (x < radius && y >= size - radius && ic(radius, size - radius - 1))
                    || (x >= size - radius
                        && y >= size - radius
                        && ic(size - radius - 1, size - radius - 1))
                {
                    pixels[idx + 3] = 0;
                    continue;
                }
                pixels[idx] = fc.r();
                pixels[idx + 1] = fc.g();
                pixels[idx + 2] = fc.b();
                pixels[idx + 3] = 255;
            }
        }
    }
    let ci = egui::ColorImage::from_rgba_unmultiplied([size as _, size as _], &pixels);
    ctx.load_texture("default_group_icon", ci, egui::TextureOptions::LINEAR)
}

fn generate_default_folder_icon(ctx: &egui::Context) -> TextureHandle {
    let size = 64i32;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let fc = Color32::from_rgb(100, 180, 255); // blue tint for folders
    let tc = Color32::from_rgb(140, 210, 255);
    let radius = size / 8;
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let ic = |cx: i32, cy: i32| -> bool {
                (x - cx) * (x - cx) + (y - cy) * (y - cy) > radius * radius
            };
            let th = 10;
            let tw = 22;
            if y < th && x < tw {
                if (x < radius && y < radius && ic(radius, radius))
                    || (x >= tw - radius && y < radius && ic(tw - radius - 1, radius))
                {
                    pixels[idx + 3] = 0;
                    continue;
                }
                pixels[idx] = tc.r();
                pixels[idx + 1] = tc.g();
                pixels[idx + 2] = tc.b();
                pixels[idx + 3] = 255;
            } else if y >= 6 {
                if (x < radius && y < 6 + radius && ic(radius, 6 + radius))
                    || (x >= size - radius && y < 6 + radius && ic(size - radius - 1, 6 + radius))
                    || (x < radius && y >= size - radius && ic(radius, size - radius - 1))
                    || (x >= size - radius
                        && y >= size - radius
                        && ic(size - radius - 1, size - radius - 1))
                {
                    pixels[idx + 3] = 0;
                    continue;
                }
                pixels[idx] = fc.r();
                pixels[idx + 1] = fc.g();
                pixels[idx + 2] = fc.b();
                pixels[idx + 3] = 255;
            }
        }
    }
    let ci = egui::ColorImage::from_rgba_unmultiplied([size as _, size as _], &pixels);
    ctx.load_texture("default_folder_icon", ci, egui::TextureOptions::LINEAR)
}

fn load_icon_from_file(
    path: &std::path::Path,
    size: u32,
    ctx: &egui::Context,
) -> Option<TextureHandle> {
    let img = image::open(path).ok()?;
    let img = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
    let rgba = img.to_rgba8().into_raw();
    let ci = egui::ColorImage::from_rgba_unmultiplied([size as _, size as _], &rgba);
    Some(ctx.load_texture(
        format!("icon_{}", path.display()),
        ci,
        egui::TextureOptions::LINEAR,
    ))
}
