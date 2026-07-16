use image::{Rgba, RgbaImage};

/// Generate a 32x32 tray icon programmatically.
///
/// Draws a simple "L" (for Launchpad) on a dark rounded-square background
/// with a subtle gradient-like look.
pub fn generate_tray_icon() -> tray_icon::Icon {
    let size = 32u32;
    let mut img = RgbaImage::new(size, size);

    let bg = Rgba([0x1F, 0x21, 0x27, 255]); // body color
    let accent = Rgba([0x80, 0x82, 0x88, 255]); // neutral accent

    // Rounded-square background
    let radius = 5.0;
    for y in 0..size {
        for x in 0..size {
            let in_corner = |cx: f32, cy: f32| -> bool {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                dx * dx + dy * dy > radius * radius
            };
            if (x as f32) < radius && (y as f32) < radius && in_corner(radius, radius)
                || (x as f32) >= size as f32 - radius
                    && (y as f32) < radius
                    && in_corner(size as f32 - radius - 1.0, radius)
                || (x as f32) < radius
                    && (y as f32) >= size as f32 - radius
                    && in_corner(radius, size as f32 - radius - 1.0)
                || (x as f32) >= size as f32 - radius
                    && (y as f32) >= size as f32 - radius
                    && in_corner(size as f32 - radius - 1.0, size as f32 - radius - 1.0)
            {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else {
                img.put_pixel(x, y, bg);
            }
        }
    }

    // Stylized "L" with two strokes
    let margin: i32 = 8;
    let left = margin;
    let right = (size as i32) - margin;
    let top = margin;
    let bot = (size as i32) - margin - 2;
    let stroke = 3;

    // Vertical stroke
    for y in top..=bot {
        for dx in 0..stroke {
            img.put_pixel((left + dx) as u32, y as u32, accent);
        }
    }
    // Horizontal stroke
    for x in left..=right {
        for dy in 0..stroke {
            img.put_pixel(x as u32, (bot - dy) as u32, accent);
        }
    }

    let (width, height) = (img.width() as _, img.height() as _);
    let rgba = img.into_raw();

    tray_icon::Icon::from_rgba(rgba, width, height).expect("Failed to create tray icon from RGBA")
}
