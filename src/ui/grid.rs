use egui::{Response, Ui, Vec2};

use crate::models::item::LaunchItem;

/// Configuration for the grid layout.
#[derive(Clone)]
pub struct GridConfig {
    pub item_size: Vec2,
    pub spacing: f32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            item_size: Vec2::new(96.0, 110.0),
            spacing: 12.0,
        }
    }
}

/// Result returned from the grid for each item interaction.
pub struct GridOutput {
    pub clicks: Vec<usize>,
    pub double_clicks: Vec<usize>,
}

fn columns(available_width: f32, item_width: f32, spacing: f32) -> usize {
    let col_w = item_width + spacing;
    let n = ((available_width + spacing) / col_w).floor() as usize;
    n.max(1)
}

/// Show a responsive grid of items.
pub fn show_grid(
    ui: &mut Ui,
    items: &[LaunchItem],
    selected_index: Option<usize>,
    config: &GridConfig,
    render_item: &mut dyn FnMut(&mut Ui, &LaunchItem, bool, Vec2) -> Response,
) -> GridOutput {
    let mut output = GridOutput {
        clicks: Vec::new(),
        double_clicks: Vec::new(),
    };

    let available = ui.available_size();
    let cols = columns(available.x, config.item_size.x, config.spacing);
    let col_w = config.item_size.x + config.spacing;

    let total_grid_w = cols as f32 * col_w - config.spacing;
    let offset_x = ((available.x - total_grid_w) / 2.0).max(0.0);

    egui::ScrollArea::vertical()
        .max_height(available.y)
        .show(ui, |ui| {
            let mut row_ui: Option<(egui::Rect, egui::Ui)> = None;

            for (i, item) in items.iter().enumerate() {
                let col = i % cols;
                let row = i / cols;

                let y = config.spacing + row as f32 * (config.item_size.y + config.spacing);

                if col == 0 {
                    row_ui.take();

                    let row_rect = egui::Rect::from_min_size(
                        egui::pos2(ui.min_rect().left(), ui.cursor().top() + y),
                        egui::vec2(available.x, config.item_size.y + config.spacing),
                    );

                    let child_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(row_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::TOP)),
                    );
                    row_ui = Some((row_rect, child_ui));
                }

                if let Some((_, ref mut child_ui)) = row_ui {
                    let item_x = if col == 0 {
                        offset_x
                    } else {
                        col as f32 * col_w
                    };
                    let desired = config.item_size;

                    let item_rect = egui::Rect::from_min_size(
                        egui::pos2(
                            child_ui.min_rect().left() + item_x,
                            child_ui.min_rect().top(),
                        ),
                        desired,
                    );

                    let mut item_ui =
                        child_ui.new_child(egui::UiBuilder::new().max_rect(item_rect).layout(
                            egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        ));

                    let is_selected = selected_index == Some(i);
                    let response = render_item(&mut item_ui, item, is_selected, desired);

                    if response.clicked() {
                        output.clicks.push(i);
                    }
                    if response.double_clicked() {
                        output.double_clicks.push(i);
                    }
                }
            }
        });

    output
}
