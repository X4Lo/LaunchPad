//! Colour themes. Built-ins are defined here; custom themes from `config.json` are
//! derived from their old-style fields over the Dark palette.

use crate::config::manager::{parse_hex, Config, Theme as CustomTheme};
use crate::{ThemeColors, ThemeOption};
use slint::Color;

fn rgb(hex: u32) -> Color {
    Color::from_rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn with_alpha(c: Color, a: u8) -> Color {
    Color::from_argb_u8(a, c.red(), c.green(), c.blue())
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::from_rgb_u8(l(a.red(), b.red()), l(a.green(), b.green()), l(a.blue(), b.blue()))
}

fn luminance(c: Color) -> f32 {
    (0.2126 * c.red() as f32 + 0.7152 * c.green() as f32 + 0.0722 * c.blue() as f32) / 255.0
}

#[allow(clippy::too_many_arguments)]
fn palette(
    dark: bool,
    bg: u32,
    surface: u32,
    hover: u32,
    sunken: u32,
    border: u32,
    border_strong: u32,
    text: u32,
    muted: u32,
    faint: u32,
    accent: u32,
    accent_strong: u32,
    danger: u32,
    success: u32,
) -> ThemeColors {
    let accent_c = rgb(accent);
    let danger_c = rgb(danger);
    ThemeColors {
        dark,
        bg: rgb(bg),
        surface: rgb(surface),
        surface_hover: rgb(hover),
        surface_sunken: rgb(sunken),
        border: rgb(border),
        border_strong: rgb(border_strong),
        text: rgb(text),
        text_muted: rgb(muted),
        text_faint: rgb(faint),
        accent: accent_c,
        accent_strong: rgb(accent_strong),
        accent_soft: with_alpha(accent_c, if dark { 0x2a } else { 0x1f }),
        on_accent: if luminance(accent_c) > 0.62 { rgb(0x12141a) } else { rgb(0xffffff) },
        danger: danger_c,
        danger_soft: with_alpha(danger_c, if dark { 0x26 } else { 0x1c }),
        success: rgb(success),
    }
}

pub const BUILTIN: [&str; 5] = ["Dark", "Light", "Dracula", "Nord", "Catppuccin"];

fn builtin(name: &str) -> Option<ThemeColors> {
    Some(match name.to_ascii_lowercase().as_str() {
        "dark" => palette(
            true, 0x0d0f13, 0x161920, 0x1c2029, 0x11141a, 0x232833, 0x323949, 0xe9ebf0, 0x8b92a1, 0x5d6473, 0x7b6bff,
            0x5b49f2, 0xff5a6a, 0x34c98a,
        ),
        "light" => palette(
            false, 0xf3f4f7, 0xffffff, 0xf7f8fa, 0xeceef2, 0xe2e5eb, 0xcfd4dc, 0x12141a, 0x687080, 0x9aa1ae, 0x6a59f5,
            0x5b49f2, 0xe5484d, 0x1f9d68,
        ),
        "dracula" => palette(
            true, 0x21222c, 0x282a36, 0x313342, 0x1d1e26, 0x343746, 0x44475a, 0xf8f8f2, 0xb3b6cc, 0x6272a4, 0xbd93f9,
            0x9e6ff2, 0xff5555, 0x50fa7b,
        ),
        "nord" => palette(
            true, 0x242933, 0x2e3440, 0x353c4a, 0x20252e, 0x3b4252, 0x4c566a, 0xeceff4, 0xaeb7c7, 0x6c788f, 0x5e81ac,
            0x4c6d96, 0xbf616a, 0xa3be8c,
        ),
        "catppuccin" => palette(
            true, 0x11111b, 0x1e1e2e, 0x262637, 0x181825, 0x313244, 0x45475a, 0xcdd6f4, 0xa6adc8, 0x6c7086, 0xcba6f7,
            0xb4befe, 0xf38ba8, 0xa6e3a1,
        ),
        _ => return None,
    })
}

/// A custom theme from config.json: the old egui fields mapped onto the token set.
fn derive_custom(t: &CustomTheme) -> ThemeColors {
    let mut c = builtin("dark").unwrap();
    let hex = |s: &Option<String>| s.as_deref().and_then(parse_hex).map(|(r, g, b)| Color::from_rgb_u8(r, g, b));
    if let Some(body) = hex(&t.body_color) {
        c.bg = body;
        c.surface_sunken = mix(body, Color::from_rgb_u8(0, 0, 0), 0.18);
        c.dark = luminance(body) < 0.5;
    }
    let lift = if c.dark { Color::from_rgb_u8(255, 255, 255) } else { Color::from_rgb_u8(0, 0, 0) };
    c.surface = hex(&t.header_color).unwrap_or_else(|| mix(c.bg, lift, 0.05));
    c.surface_hover = hex(&t.widget_color).unwrap_or_else(|| mix(c.surface, lift, 0.06));
    c.border = hex(&t.divider_color).unwrap_or_else(|| mix(c.surface, lift, 0.08));
    c.border_strong = mix(c.border, lift, 0.12);
    if let Some(text) = hex(&t.text_color) {
        c.text = text;
    } else if !c.dark {
        c.text = rgb(0x12141a);
    }
    c.text_muted = mix(c.text, c.bg, 0.4);
    c.text_faint = mix(c.text, c.bg, 0.62);
    if let Some(accent) = hex(&t.selection_color) {
        c.accent = accent;
        c.accent_strong = mix(accent, Color::from_rgb_u8(0, 0, 0), 0.15);
        c.accent_soft = with_alpha(accent, 0x2a);
        c.on_accent = if luminance(accent) > 0.62 { rgb(0x12141a) } else { rgb(0xffffff) };
    }
    c
}

/// All selectable themes: built-ins then custom ones (by name).
pub fn names(cfg: &Config) -> Vec<String> {
    let mut v: Vec<String> = BUILTIN.iter().map(|s| s.to_string()).collect();
    v.extend(cfg.themes.iter().map(|t| t.name.clone()).filter(|n| !BUILTIN.iter().any(|b| b.eq_ignore_ascii_case(n))));
    v
}

pub fn colors_for(cfg: &Config, name: &str) -> ThemeColors {
    builtin(name)
        .or_else(|| cfg.themes.iter().find(|t| t.name.eq_ignore_ascii_case(name)).map(derive_custom))
        .unwrap_or_else(|| builtin("dark").unwrap())
}

/// The active theme's name (Dark when unset or unknown).
pub fn current_name(cfg: &Config) -> String {
    let names = names(cfg);
    cfg.selected_theme
        .as_deref()
        .and_then(|sel| names.iter().find(|n| n.eq_ignore_ascii_case(sel)).cloned())
        .unwrap_or_else(|| "Dark".into())
}

pub fn current(cfg: &Config) -> ThemeColors {
    colors_for(cfg, &current_name(cfg))
}

pub fn options(cfg: &Config) -> Vec<ThemeOption> {
    names(cfg)
        .into_iter()
        .map(|name| {
            let c = colors_for(cfg, &name);
            let custom = !BUILTIN.iter().any(|b| b.eq_ignore_ascii_case(&name));
            ThemeOption { name: name.into(), bg: c.bg, surface: c.surface, accent: c.accent, text: c.text, custom }
        })
        .collect()
}
