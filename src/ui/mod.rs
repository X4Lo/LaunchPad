//! Everything visual on the Rust side: the generated Slint components live in the crate
//! root (`slint::include_modules!`); this module holds the app icon, themes and helpers.

pub mod icon;
pub mod theme;

/// Deterministic tile colour for items without an icon.
pub fn tint_for(key: &str) -> slint::Color {
    const PALETTE: [(u8, u8, u8); 8] = [
        (0x7b, 0x6b, 0xff),
        (0x2f, 0xa8, 0xe0),
        (0x24, 0xb3, 0x8a),
        (0xe0, 0x8a, 0x2f),
        (0xd9, 0x4f, 0x8f),
        (0x5f, 0x7d, 0xf0),
        (0xc0, 0x5c, 0xe0),
        (0x3f, 0xa0, 0x6a),
    ];
    let h = key.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let (r, g, b) = PALETTE[(h as usize) % PALETTE.len()];
    slint::Color::from_rgb_u8(r, g, b)
}

pub fn initial_for(name: &str) -> String {
    name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into())
}

pub fn image_from_rgba(width: u32, height: u32, pixels: &[u8]) -> slint::Image {
    let buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(pixels, width, height);
    slint::Image::from_rgba8(buf)
}

pub fn app_icon_image(size: u32) -> slint::Image {
    image_from_rgba(size, size, &icon::render(size))
}

fn winit_icon(size: u32) -> Option<slint::winit_030::winit::window::Icon> {
    slint::winit_030::winit::window::Icon::from_rgba(icon::render(size), size, size).ok()
}

/// Title bar / Alt+Tab / taskbar icons and the light/dark title bar. Slint can't turn
/// pixel-buffer images into native window icons itself, so set them on the winit window.
pub fn apply_native_chrome(w: &slint::winit_030::winit::window::Window, dark: bool) {
    use slint::winit_030::winit::window::Theme as WinitTheme;
    w.set_theme(Some(if dark { WinitTheme::Dark } else { WinitTheme::Light }));
    w.set_window_icon(winit_icon(32));
    #[cfg(windows)]
    {
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        w.set_taskbar_icon(winit_icon(64));
    }
}

/// Largest icon size (logical px, multiple of 4, 32–160) at which `count` tiles fit in
/// `width` × `height`. Mirrors the cell geometry in `ui/main.slint`.
pub fn fit_icon_size(width: f32, height: f32, count: usize, gap: f32) -> f32 {
    if count == 0 || width <= 0.0 || height <= 0.0 {
        return 56.0;
    }
    let mut best = 32.0;
    let mut size = 32.0;
    while size <= 160.0 {
        let cell_w = (size + 36.0f32).max(88.0);
        let cell_h = size + 44.0;
        let cols = (((width + gap) / (cell_w + gap)).floor() as usize).max(1);
        let rows = count.div_ceil(cols);
        let needed_h = rows as f32 * cell_h + rows.saturating_sub(1) as f32 * gap;
        if needed_h <= height {
            best = size;
        }
        size += 4.0;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_grows_for_few_items_and_shrinks_for_many() {
        let few = fit_icon_size(700.0, 400.0, 3, 12.0);
        let many = fit_icon_size(700.0, 400.0, 60, 12.0);
        assert!(few > many, "{few} vs {many}");
        assert!((32.0..=160.0).contains(&many));
    }

    #[test]
    fn initials() {
        assert_eq!(initial_for("  steam"), "S");
        assert_eq!(initial_for("—"), "?");
    }
}
