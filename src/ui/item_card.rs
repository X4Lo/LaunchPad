use egui::{Color32, CornerRadius, Response, Stroke, Ui, Vec2};

use crate::models::item::LaunchItem;

use super::icons::IconCache;

/// Render a single item card (icon + title).
///
/// Returns the response so the caller can check for clicks.
pub fn render_item_card(
    ui: &mut Ui,
    item: &LaunchItem,
    is_selected: bool,
    desired_size: Vec2,
    icon_cache: &mut IconCache,
) -> Response {
    let icon_size = 48u32;
    let key = IconCache::key_for(item, icon_size);
    let icon_texture = icon_cache.get_or_load(key, ui.ctx());

    let card_rounding = CornerRadius::same(8);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    // Background on hover or selection
    if is_selected || response.hovered() {
        let bg_color = if is_selected {
            Color32::from_rgba_premultiplied(80, 80, 120, 180)
        } else {
            Color32::from_rgba_premultiplied(60, 60, 80, 120)
        };
        ui.painter().rect_filled(rect, card_rounding, bg_color);
    }

    // Border when selected
    if is_selected {
        let border = Stroke::new(2.0_f32, Color32::from_rgb(137, 180, 250));
        ui.painter().rect_stroke(rect, card_rounding, border, egui::StrokeKind::Inside);
    }

    // Icon
    if let Some(tex) = icon_texture {
        let icon_rect = egui::Rect::from_center_size(
            rect.center() - egui::vec2(0.0, 10.0),
            egui::vec2(icon_size as f32, icon_size as f32),
        );
        ui.painter().image(
            tex.id(),
            icon_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // Label
    let label_color = if is_selected {
        Color32::WHITE
    } else {
        Color32::from_gray(220)
    };

    let label_y = rect.center().y + icon_size as f32 / 2.0 + 6.0;

    let title = item.title();
    let display_title = if title.len() > 14 {
        format!("{}…", &title[..13])
    } else {
        title.to_string()
    };

    ui.painter().text(
        egui::pos2(rect.center().x, label_y),
        egui::Align2::CENTER_TOP,
        &display_title,
        egui::FontId::proportional(12.0),
        label_color,
    );

    response
}
