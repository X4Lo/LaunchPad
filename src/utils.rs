use image::{Rgba, RgbaImage};

/// Generate a simple 32x32 tray icon programmatically.
///
/// This avoids needing an external PNG file packaged with the binary.
/// Creates a stylized "L" (for Launchpad) on a rounded-square background.
pub fn generate_tray_icon() -> tray_icon::Icon {
    let size = 32u32;
    let mut img = RgbaImage::new(size, size);

    let bg = Rgba([30, 30, 46, 255]);       // dark background (#1E1E2E)
    let accent = Rgba([137, 180, 250, 255]); // soft blue (#89B4FA)

    // Fill with background
    for y in 0..size {
        for x in 0..size {
            // Rounded corners: skip pixels in the corners
            let cx = x as f32 - size as f32 / 2.0;
            let cy = y as f32 - size as f32 / 2.0;
            let radius = size as f32 / 2.0 - 1.0;
            if cx * cx + cy * cy > radius * radius {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else {
                img.put_pixel(x, y, bg);
            }
        }
    }

    // Draw a simple "L" shape
    let left = 8;
    let right = 24;
    let top = 8;
    let bottom = 24;

    for y in top..bottom {
        img.put_pixel(left, y, accent);
    }
    for x in left..=right {
        img.put_pixel(x, bottom, accent);
    }

    // Convert to RGBA bytes
    let (width, height) = (img.width() as _, img.height() as _);
    let rgba = img.into_raw();

    tray_icon::Icon::from_rgba(rgba, width, height).expect("Failed to create tray icon from RGBA")
}
