use egui::{CornerRadius, Visuals};

use crate::config::manager::{Config, Theme};

/// Apply the Launchpad visual theme from config.
/// Falls back to built-in defaults if no theme is selected.
pub fn apply_theme(ctx: &egui::Context, config: &Config) {
    let theme = config.resolve_theme();

    let mut visuals = Visuals::dark();

    // Resolve colors from theme
    let _header_color =
        hex_opt(&theme.header_color).unwrap_or(egui::Color32::from_rgb(0x31, 0x33, 0x37));
    let body_color =
        hex_opt(&theme.body_color).unwrap_or(egui::Color32::from_rgb(0x1F, 0x21, 0x27));
    let widget_color =
        hex_opt(&theme.widget_color).unwrap_or(egui::Color32::from_rgb(0x3A, 0x3C, 0x42));
    let selection_color =
        hex_opt(&theme.selection_color).unwrap_or(egui::Color32::from_rgb(0x60, 0x62, 0x68));

    visuals.window_fill = body_color;
    visuals.panel_fill = body_color;
    visuals.faint_bg_color = widget_color;
    visuals.extreme_bg_color = egui::Color32::from_rgb(0x18, 0x1A, 0x1F);

    let radius = theme.corner_radius.unwrap_or(12);
    visuals.window_corner_radius = CornerRadius::same(radius);
    visuals.menu_corner_radius = CornerRadius::same(radius.max(8));

    let rounding = CornerRadius::same(6);
    visuals.widgets.noninteractive.corner_radius = rounding;
    visuals.widgets.inactive.corner_radius = rounding;
    visuals.widgets.hovered.corner_radius = rounding;
    visuals.widgets.active.corner_radius = rounding;

    visuals.widgets.inactive.bg_fill = widget_color;
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x48, 0x4A, 0x50);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0x55, 0x57, 0x5E);

    visuals.selection.bg_fill = selection_color.linear_multiply(0.3);

    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4].into(),
        blur: 24,
        spread: 0,
        color: egui::Color32::from_black_alpha(160),
    };

    visuals.striped = false;
    visuals.indent_has_left_vline = false;

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);
}

fn hex_opt(s: &Option<String>) -> Option<egui::Color32> {
    Theme::parse_hex(s.as_deref()?)
}
