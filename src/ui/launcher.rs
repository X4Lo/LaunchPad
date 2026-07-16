use egui::{Color32, CornerRadius, FontId, Ui};

use crate::config::manager::Config;
use crate::models::item::{ItemId, LaunchItem};
use crate::ui::grid::{self, GridConfig};
use crate::ui::icons::IconCache;
use crate::ui::item_card;

/// Renders the full launcher UI: navigation bar + grid at the current level.
pub struct LauncherUI;

impl LauncherUI {
    /// Show the launcher for the current navigation state.
    pub fn show(
        ui: &mut Ui,
        config: &Config,
        nav_stack: &[ItemId],
        selected_index: Option<usize>,
        icon_cache: &mut IconCache,
    ) -> LauncherResponse {
        let mut response = LauncherResponse::default();

        // --- Navigation bar ---
        Self::render_nav_bar(ui, config, nav_stack);

        ui.separator();

        // --- Get items at current level ---
        let items: &[LaunchItem] = if let Some(&group_id) = nav_stack.last() {
            let group = config.items.iter().find_map(|item| {
                if let LaunchItem::Group(g) = item {
                    if g.id == group_id {
                        Some(g)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            group.map(|g| g.items.as_slice()).unwrap_or(&[])
        } else {
            config.items.as_slice()
        };

        if items.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Empty")
                        .color(Color32::from_gray(150))
                        .size(16.0),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Drag & drop executables here")
                        .color(Color32::from_gray(120))
                        .size(13.0),
                );
            });
            return response;
        }

        // --- Grid ---
        let item_size = egui::Vec2::new(96.0, 110.0);
        let spacing = 14.0;
        let config_grid = GridConfig { item_size, spacing };

        let grid_output = grid::show_grid(
            ui,
            items,
            selected_index,
            &config_grid,
            &mut |ui, item, is_selected, desired_size| {
                item_card::render_item_card(ui, item, is_selected, desired_size, icon_cache)
            },
        );

        // Map grid output to launcher response
        for &i in &grid_output.clicks {
            if let Some(item) = items.get(i) {
                response.clicked_index = Some(i);
                response.clicked_id = Some(item.id());
            }
        }

        for &i in &grid_output.double_clicks {
            if let Some(item) = items.get(i) {
                response.double_clicked_id = Some(item.id());
            }
        }

        response
    }

    /// Render the navigation bar showing the current path.
    fn render_nav_bar(ui: &mut Ui, config: &Config, nav_stack: &[ItemId]) {
        let height = 36.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::hover(),
        );

        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgba_premultiplied(20, 20, 30, 200),
        );

        let mut x = rect.left() + 12.0;
        let y = rect.center().y;

        let home_text = "Launchpad";
        let home_galley = ui.painter().layout_no_wrap(
            home_text.to_string(),
            FontId::proportional(13.0),
            Color32::from_rgb(137, 180, 250),
        );
        let home_size = home_galley.size();
        ui.painter().galley(
            egui::pos2(x, y - home_size.y / 2.0),
            home_galley,
            Color32::PLACEHOLDER,
        );
        x += home_size.x;

        for &group_id in nav_stack {
            let name = find_group_title(config, group_id).unwrap_or_default();

            let sep = "  ▸  ";
            let sep_galley = ui.painter().layout_no_wrap(
                sep.to_string(),
                FontId::proportional(13.0),
                Color32::from_gray(120),
            );
            let sep_size = sep_galley.size();
            ui.painter().galley(
                egui::pos2(x, y - sep_size.y / 2.0),
                sep_galley,
                Color32::PLACEHOLDER,
            );
            x += sep_size.x;

            let name_galley = ui.painter().layout_no_wrap(
                name,
                FontId::proportional(13.0),
                Color32::from_gray(220),
            );
            let name_size = name_galley.size();
            ui.painter().galley(
                egui::pos2(x, y - name_size.y / 2.0),
                name_galley,
                Color32::PLACEHOLDER,
            );
            x += name_size.x;
        }
    }
}

/// Responses collected during launcher rendering.
#[derive(Default)]
pub struct LauncherResponse {
    pub clicked_index: Option<usize>,
    pub clicked_id: Option<ItemId>,
    pub double_clicked_id: Option<ItemId>,
}

fn find_group_title(config: &Config, id: ItemId) -> Option<String> {
    config.items.iter().find_map(|item| {
        if let LaunchItem::Group(g) = item {
            if g.id == id {
                Some(g.title.clone())
            } else {
                None
            }
        } else {
            None
        }
    })
}
