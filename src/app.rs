use crossbeam::channel::Receiver;
use egui::Context;
use std::sync::{Mutex, OnceLock};

use crate::commands;
use crate::config::manager::{Config, ConfigManager};
use crate::models::item::{ItemId, LaunchItem};
use crate::platform::hotkey;
use crate::platform::tray::TrayEvent;
use crate::ui::icons::IconCache;

/// Global egui context so background threads can wake the UI.
static UI_CTX: OnceLock<Mutex<Context>> = OnceLock::new();

/// Wake egui's event loop from any thread (hotkey, tray, etc).
pub fn wake_ui() {
    if let Some(m) = UI_CTX.get() {
        if let Ok(ctx) = m.lock() {
            ctx.request_repaint();
        }
    }
}

pub struct LaunchpadApp {
    hotkey_rx: Receiver<()>,
    tray_rx: Receiver<TrayEvent>,
    config: Config,
    config_manager: ConfigManager,
    dirty: bool,
    nav_stack: Vec<ItemId>,
    selected_index: Option<usize>,
    icon_cache: IconCache,
    show_settings: bool,
    context_menu: Option<ContextMenuState>,
    pos_restored: bool,
    pending_rename: Option<(ItemId, String)>,
    pending_group_select: Option<ItemId>,
    pending_delete_group: Option<ItemId>,
    resizing: Option<ResizeEdge>,
    resize_start: Option<egui::Pos2>,
    show_reorder: bool,
}

#[derive(Clone)]
struct ContextMenuState {
    item_index: usize,
    pos: egui::Pos2,
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum ResizeEdge {
    Bottom,
    Right,
    Corner,
}

impl LaunchpadApp {
    pub fn new(
        hotkey_rx: Receiver<()>,
        tray_rx: Receiver<TrayEvent>,
        config: Config,
        config_manager: ConfigManager,
    ) -> Self {
        Self {
            hotkey_rx,
            tray_rx,
            config,
            config_manager,
            dirty: false,
            nav_stack: Vec::new(),
            selected_index: None,
            icon_cache: IconCache::new(),
            show_settings: false,
            context_menu: None,
            pos_restored: false,
            pending_rename: None,
            pending_group_select: None,
            pending_delete_group: None,
            resizing: None,
            resize_start: None,
            show_reorder: false,
        }
    }
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    fn save_if_dirty(&mut self) {
        if self.dirty {
            if let Err(e) = self.config_manager.save(&self.config) {
                log::error!("Save: {}", e);
            } else {
                self.dirty = false;
            }
        }
    }
    fn current_items(&self) -> &[LaunchItem] {
        if let Some(&gid) = self.nav_stack.last() {
            for item in &self.config.items {
                if let LaunchItem::Group(g) = item {
                    if g.id == gid {
                        return &g.items;
                    }
                }
            }
            &[]
        } else {
            &self.config.items
        }
    }
    fn is_at_root(&self) -> bool {
        self.nav_stack.is_empty()
    }
    fn navigate_to_root(&mut self) {
        self.nav_stack.clear();
        self.selected_index = None;
        self.context_menu = None;
    }
    fn navigate_into(&mut self, gid: ItemId) {
        self.nav_stack.push(gid);
        self.selected_index = None;
        self.context_menu = None;
    }
    fn navigate_back(&mut self) {
        self.nav_stack.pop();
        self.selected_index = None;
        self.context_menu = None;
    }
    fn activate_item(&mut self, item: &LaunchItem) {
        match item {
            LaunchItem::App(a) => {
                let _ = std::process::Command::new(&a.executable_path).spawn();
                if self.config.hide_on_launch {
                    self.minimize();
                }
            }
            LaunchItem::Group(g) => self.navigate_into(g.id),
            LaunchItem::Folder(f) => {
                let _ = std::process::Command::new("explorer")
                    .arg(&f.folder_path)
                    .spawn();
                if self.config.hide_on_launch {
                    self.minimize();
                }
            }
        }
    }
    fn minimize(&self) {
        if let Some(m) = UI_CTX.get() {
            if let Ok(ctx) = m.lock() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }
    }
    fn has_custom_icon(&self, id: ItemId) -> bool {
        for item in &self.config.items {
            match item {
                LaunchItem::App(a) if a.id == id => return a.icon_path.is_some(),
                LaunchItem::Group(g) if g.id == id => return g.icon_path.is_some(),
                LaunchItem::Folder(f) if f.id == id => return f.icon_path.is_some(),
                _ => {}
            }
        }
        for item in &self.config.items {
            if let LaunchItem::Group(g) = item {
                for sub in &g.items {
                    match sub {
                        LaunchItem::App(a) if a.id == id => return a.icon_path.is_some(),
                        LaunchItem::Folder(f) if f.id == id => return f.icon_path.is_some(),
                        _ => {}
                    }
                }
            }
        }
        false
    }
}

impl eframe::App for LaunchpadApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        UI_CTX.get_or_init(|| Mutex::new(ctx.clone()));

        // Process hotkey — toggle window visibility
        let mut hotkey_fired = false;
        while self.hotkey_rx.try_recv().is_ok() {
            hotkey_fired = true;
        }
        if hotkey_fired {
            log::info!("Processing hotkey toggle");
            let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
            if minimized {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }

        // Process tray events
        while let Ok(ev) = self.tray_rx.try_recv() {
            log::info!("Processing tray event: {:?}", ev);
            match ev {
                TrayEvent::Toggle => {
                    if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                }
                TrayEvent::Quit => {
                    self.save_if_dirty();
                    std::process::exit(0);
                }
            }
        }
        if !self.pos_restored {
            self.pos_restored = true;
            if let (Some(x), Some(y)) = (self.config.window_x, self.config.window_y) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(x, y)));
            }
            if let (Some(w), Some(h)) = (self.config.window_width, self.config.window_height) {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(w, h)));
            }
        }
        // Persist window geometry whenever it changes
        ctx.input(|i| {
            if let Some(r) = i.viewport().outer_rect {
                let nx = r.min.x;
                let ny = r.min.y;
                if self.config.window_x != Some(nx) || self.config.window_y != Some(ny) {
                    self.config.window_x = Some(nx);
                    self.config.window_y = Some(ny);
                    self.mark_dirty();
                }
            }
            if let Some(r) = i.viewport().inner_rect {
                let nw = r.width();
                let nh = r.height();
                if self.config.window_width != Some(nw) || self.config.window_height != Some(nh) {
                    self.config.window_width = Some(nw);
                    self.config.window_height = Some(nh);
                    self.mark_dirty();
                }
            }
        });
        self.handle_dropped_files(ctx);
        // Resize handling
        if let Some(edge) = self.resizing {
            if ctx.input(|i| i.pointer.any_released()) {
                self.resizing = None;
                self.resize_start = None;
            } else if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                if let Some(start) = self.resize_start {
                    let delta = pos - start;
                    let cur = ctx.input(|i| {
                        i.viewport()
                            .inner_rect
                            .map(|r| r.size())
                            .unwrap_or(egui::vec2(640.0, 480.0))
                    });
                    let min = egui::vec2(200.0, 200.0);
                    match edge {
                        ResizeEdge::Right => {
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                                (cur.x + delta.x).max(min.x),
                                cur.y,
                            )))
                        }
                        ResizeEdge::Bottom => {
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                                cur.x,
                                (cur.y + delta.y).max(min.y),
                            )))
                        }
                        ResizeEdge::Corner => {
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                                (cur.x + delta.x).max(min.x),
                                (cur.y + delta.y).max(min.y),
                            )))
                        }
                    }
                    self.resize_start = Some(pos);
                }
            }
        }
        self.save_if_dirty();
        crate::ui::theme::apply_theme(ctx, &self.config);
        let theme = self.config.resolve_theme();
        let header_color =
            hex_opt(&theme.header_color).unwrap_or(egui::Color32::from_rgb(0x31, 0x33, 0x37));
        let accent_color =
            hex_opt(&theme.divider_color).unwrap_or(egui::Color32::from_rgb(0x3D, 0x3F, 0x43));
        egui::TopBottomPanel::top("title_bar")
            .frame(
                egui::Frame::NONE
                    .fill(header_color)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| self.render_title_bar(ui, accent_color));
        if self.show_settings {
            egui::Window::new("Settings")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| self.render_settings(ui));
        }
        if let Some(ref cm) = self.context_menu.clone() {
            self.render_context_menu(ctx, cm);
        }
        if let Some((id, ref mut text)) = self.pending_rename.clone() {
            let mut t = text.clone();
            let item_id = id;
            egui::Window::new("Rename")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("New name:");
                    ui.text_edit_singleline(&mut t);
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            let _ =
                                commands::items::rename_item(&mut self.config, item_id, t.clone());
                            self.mark_dirty();
                            self.pending_rename = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_rename = None;
                        }
                    });
                });
            if let Some((_, ref mut stored)) = self.pending_rename {
                *stored = t;
            }
        }
        if let Some(item_id) = self.pending_group_select {
            self.render_group_selector(ctx, item_id);
        }
        if let Some(gid) = self.pending_delete_group {
            let group_title = group_name(&self.config, gid);
            let item_count = self
                .config
                .items
                .iter()
                .filter_map(|i| {
                    if let LaunchItem::Group(g) = i {
                        if g.id == gid {
                            Some(g.items.len())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .next()
                .unwrap_or(0);
            egui::Window::new("Confirm Delete")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Delete group \"{}\"?", group_title));
                    ui.label(format!(
                        "It contains {} item(s). This cannot be undone.",
                        item_count
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            let _ = commands::items::delete_group(&mut self.config, gid);
                            if self.nav_stack.last() == Some(&gid) {
                                self.navigate_back();
                            }
                            self.mark_dirty();
                            self.pending_delete_group = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_delete_group = None;
                        }
                    });
                });
        }
        if self.show_reorder {
            self.render_reorder_view(ctx);
        }
        let is_dragging = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if is_dragging {
            egui::Area::new("drag_overlay".into())
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let rect = ui.ctx().screen_rect();
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::ZERO,
                        egui::Color32::from_rgba_premultiplied(0x60, 0x62, 0x68, 40),
                    );
                    ui.painter().rect_stroke(
                        rect,
                        egui::CornerRadius::ZERO,
                        egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(0x80, 0x82, 0x88)),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Drop files here to add",
                        egui::FontId::proportional(18.0),
                        egui::Color32::WHITE,
                    );
                });
        }
        // Click-away: close context menu
        if self.context_menu.is_some()
            && ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
        {
            self.context_menu = None;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.is_at_root(), "Launchpad")
                    .clicked()
                {
                    self.navigate_to_root();
                }
                for (i, &gid) in self.nav_stack.iter().enumerate() {
                    ui.label(">");
                    let (name, color) = group_name_with_icon(&self.config, gid);
                    // Draw small colored square as type indicator
                    let (sq, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(sq, egui::CornerRadius::same(2), color);
                    let label = name;
                    if i == self.nav_stack.len() - 1 {
                        ui.label(egui::RichText::new(label).strong());
                    } else {
                        ui.label(label);
                    }
                }
            });
            // Custom subtle divider
            let div_color = hex_opt(&self.config.resolve_theme().divider_color)
                .unwrap_or(egui::Color32::from_rgb(0x3D, 0x3F, 0x43));
            let div_y = ui.cursor().top();
            ui.add_space(6.0);
            let div_rect = egui::Rect::from_min_max(
                egui::pos2(ui.min_rect().left() + 12.0, div_y),
                egui::pos2(ui.max_rect().right() - 12.0, div_y + 1.0),
            );
            ui.painter()
                .rect_filled(div_rect, egui::CornerRadius::ZERO, div_color);
            ui.add_space(6.0);
            let items: Vec<LaunchItem> = self.current_items().to_vec();
            if items.is_empty() {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Empty")
                            .color(egui::Color32::from_gray(150))
                            .size(16.0),
                    );
                    if ui.button("Add Demo Items").clicked() {
                        self.add_demo_items();
                    }
                });
            } else {
                self.render_grid(ui, &items);
            }
        });
        // Resize handles — rendered as an overlay so they're always reachable
        self.render_resize_handles(ctx);
        self.handle_keyboard(ctx);
        // Poll regularly so hotkey/tray events are picked up
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

// ─── Resize Handles ──────────────────────────────────────
impl LaunchpadApp {
    fn render_resize_handles(&mut self, ctx: &egui::Context) {
        let sz = 6.0;
        let rect = ctx.screen_rect();
        // Safety: if resizing got stuck (e.g. mouse released outside window), reset it
        if self.resizing.is_some()
            && !ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down())
        {
            self.resizing = None;
            self.resize_start = None;
        }

        // Right edge strip
        let r_r = egui::Rect::from_min_max(
            egui::pos2(rect.right() - sz, rect.top()),
            egui::pos2(rect.right(), rect.bottom() - sz),
        );
        // Bottom edge strip
        let b_r = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - sz),
            egui::pos2(rect.right() - sz, rect.bottom()),
        );
        // Corner
        let c_r = egui::Rect::from_min_max(
            egui::pos2(rect.right() - sz, rect.bottom() - sz),
            egui::pos2(rect.right(), rect.bottom()),
        );

        for (edge, r) in [
            (ResizeEdge::Right, r_r),
            (ResizeEdge::Bottom, b_r),
            (ResizeEdge::Corner, c_r),
        ] {
            let id = egui::Id::new(format!("resize_handle_{:?}", edge));
            let area = egui::Area::new(id).fixed_pos(r.min).interactable(true);
            area.show(ctx, |ui| {
                ui.set_min_size(r.size());
                let resp = ui.interact(ui.max_rect(), ui.next_auto_id(), egui::Sense::drag());
                if resp.dragged() {
                    self.resizing = Some(edge);
                    self.resize_start = ctx.input(|i| i.pointer.hover_pos());
                }
                let hover = resp.hovered() || self.resizing == Some(edge);
                if hover {
                    let c = match edge {
                        ResizeEdge::Corner => egui::CursorIcon::ResizeNwSe,
                        ResizeEdge::Right => egui::CursorIcon::ResizeEast,
                        ResizeEdge::Bottom => egui::CursorIcon::ResizeSouth,
                    };
                    ui.ctx().set_cursor_icon(c);
                }
            });
        }
    }
}

// ─── Title Bar ───────────────────────────────────────────
impl LaunchpadApp {
    fn render_title_bar(&mut self, ui: &mut egui::Ui, line_color: egui::Color32) {
        let h = 32.0;
        let bounds = egui::Rect::from_min_size(
            ui.max_rect().left_top(),
            egui::vec2(ui.max_rect().width(), h),
        );
        ui.allocate_space(egui::vec2(ui.available_width(), h));
        let drag = ui.interact(bounds, ui.next_auto_id(), egui::Sense::drag());
        if drag.dragged() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let p = ui.painter().clone();
        // Bottom accent line
        p.line_segment(
            [
                egui::pos2(bounds.left(), bounds.bottom()),
                egui::pos2(bounds.right(), bounds.bottom()),
            ],
            egui::Stroke::new(1.0_f32, line_color),
        );
        p.text(
            bounds.center(),
            egui::Align2::CENTER_CENTER,
            "Launchpad",
            egui::FontId::proportional(13.0),
            egui::Color32::from_gray(160),
        );
        // Reorder button — draw up/down arrows
        let rr = egui::Rect::from_min_size(
            egui::pos2(bounds.right() - 100.0, bounds.top() + 4.0),
            egui::vec2(24.0, 24.0),
        );
        if ui.rect_contains_pointer(rr)
            && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
        {
            self.show_reorder = !self.show_reorder;
        }
        let reorder_color = if self.show_reorder {
            egui::Color32::from_rgb(0xF2, 0xC9, 0x4C)
        } else {
            egui::Color32::from_gray(180)
        };
        draw_updown_arrows(&p, rr, reorder_color);
        // Settings gear — draw a simple cog
        let sr = egui::Rect::from_min_size(
            egui::pos2(bounds.right() - 68.0, bounds.top() + 4.0),
            egui::vec2(24.0, 24.0),
        );
        if ui.rect_contains_pointer(sr)
            && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
        {
            self.show_settings = !self.show_settings;
        }
        draw_cog(&p, sr, egui::Color32::WHITE);
        let cr = egui::Rect::from_min_size(
            egui::pos2(bounds.right() - 36.0, bounds.top() + 4.0),
            egui::vec2(24.0, 24.0),
        );
        let ch = ui.rect_contains_pointer(cr);
        if ch && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
            self.save_if_dirty();
            std::process::exit(0);
        }
        let color = if ch {
            egui::Color32::from_rgb(255, 80, 80)
        } else {
            egui::Color32::from_gray(180)
        };
        let inset = 6.0;
        p.line_segment(
            [
                cr.left_top() + egui::vec2(inset, inset),
                cr.right_bottom() - egui::vec2(inset, inset),
            ],
            egui::Stroke::new(2.0_f32, color),
        );
        p.line_segment(
            [
                cr.right_top() + egui::vec2(-inset, inset),
                cr.left_bottom() + egui::vec2(inset, -inset),
            ],
            egui::Stroke::new(2.0_f32, color),
        );
    }
}

// ─── Settings ────────────────────────────────────────────
impl LaunchpadApp {
    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.separator();

        // ── General ──
        ui.collapsing("General", |ui| {
            ui.label("Grid Spacing");
            if ui
                .add(egui::Slider::new(&mut self.config.grid_spacing, 2.0..=24.0).text("px"))
                .changed()
            {
                self.mark_dirty();
            }
            ui.label("Icon Size");
            if ui
                .add(egui::Slider::new(&mut self.config.grid_icon_size, 24.0..=72.0).text("px"))
                .changed()
            {
                self.mark_dirty();
            }
            ui.separator();
            if ui
                .checkbox(
                    &mut self.config.hide_on_launch,
                    "Hide Launchpad on app launch",
                )
                .changed()
            {
                self.mark_dirty();
            }
            ui.label("Minimizes after opening an app or folder.");
            ui.separator();
            ui.label("Global Hotkey");
            ui.label("Format: Ctrl+Alt+R, Ctrl+Shift+F, etc.");
            let mut hk = self.config.hotkey.clone();
            if ui.text_edit_singleline(&mut hk).changed() {
                // Validate the new hotkey
                if hotkey::parse_hotkey(&hk).is_some() {
                    self.config.hotkey = hk;
                    self.mark_dirty();
                    ui.label(
                        egui::RichText::new("Valid — restart to apply")
                            .color(egui::Color32::from_rgb(100, 200, 100)),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("Invalid hotkey format")
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                }
            }
            ui.label("Changes take effect after restart.");
        });

        // ── Themes ──
        ui.collapsing("Themes", |ui| {
            let current = self.config.selected_theme.clone();
            ui.label("Select a theme:");
            ui.add_space(4.0);
            // "None" option for defaults
            let none_label = "Default";
            let is_none = current.is_none();
            if ui.selectable_label(is_none, none_label).clicked() {
                self.config.selected_theme = None;
                self.mark_dirty();
            }
            for theme in &self.config.themes.clone() {
                let is_sel = current.as_deref() == Some(&theme.name);
                // Show a small color preview
                let preview = if theme.header_color.is_some() {
                    format!("  ■ {}", theme.name)
                } else {
                    theme.name.clone()
                };
                if ui.selectable_label(is_sel, preview).clicked() {
                    self.config.selected_theme = Some(theme.name.clone());
                    self.mark_dirty();
                }
            }
            // Color swatches for the selected theme
            if let Some(ref name) = current {
                let t = self.config.resolve_theme();
                ui.add_space(8.0);
                ui.label(format!("Preview: {}", name));
                ui.horizontal(|ui| {
                    if let Some(ref h) = t.header_color {
                        swatch(ui, "H", h);
                    }
                    if let Some(ref b) = t.body_color {
                        swatch(ui, "B", b);
                    }
                    if let Some(ref w) = t.widget_color {
                        swatch(ui, "W", w);
                    }
                    if let Some(ref s) = t.selection_color {
                        swatch(ui, "S", s);
                    }
                });
            }
        });

        ui.add_space(16.0);
        if ui.button("Close").clicked() {
            self.show_settings = false;
        }
    }
}

// ─── Context Menu ───────────────────────────────────────
impl LaunchpadApp {
    fn render_context_menu(&mut self, ctx: &egui::Context, state: &ContextMenuState) {
        let items: Vec<LaunchItem> = self.current_items().to_vec();
        let item = match items.get(state.item_index) {
            Some(i) => i.clone(),
            None => {
                self.context_menu = None;
                return;
            }
        };
        let id = item.id();
        let is_group = matches!(&item, LaunchItem::Group(_));
        let has_custom = self.has_custom_icon(id);
        egui::Window::new("context_menu")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(state.pos)
            .default_width(150.0)
            .show(ctx, |ui| {
                ui.set_min_width(140.0);
                ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);
                let ol = match &item {
                    LaunchItem::Folder(_) => "Open Folder",
                    _ => "Open",
                };
                if ui.button(ol).clicked() {
                    self.activate_item(&item);
                    self.context_menu = None;
                }
                ui.separator();
                if ui.button("Rename").clicked() {
                    self.pending_rename = Some((id, item.title().to_string()));
                    self.context_menu = None;
                }
                if ui.button("Change Icon").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "ico", "jpg", "bmp"])
                        .pick_file()
                    {
                        // Copy the icon to the local icons folder
                        let icons_dir = self.config_manager.icons_dir();
                        let _ = std::fs::create_dir_all(&icons_dir);
                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
                        let dest = icons_dir.join(format!("{}.{}", uuid::Uuid::new_v4(), ext));
                        if std::fs::copy(&path, &dest).is_ok() {
                            let _ = commands::items::set_icon(&mut self.config, id, dest);
                            self.mark_dirty();
                        } else {
                            log::error!(
                                "Failed to copy icon from {} to {}",
                                path.display(),
                                dest.display()
                            );
                        }
                    }
                    self.context_menu = None;
                }
                if has_custom {
                    if ui.button("Remove Icon").clicked() {
                        let _ = commands::items::clear_icon(&mut self.config, id);
                        self.mark_dirty();
                        self.context_menu = None;
                    }
                }
                if !is_group {
                    if ui.button("Add to Group").clicked() {
                        self.pending_group_select = Some(id);
                        self.context_menu = None;
                    }
                    if !self.is_at_root() {
                        if ui.button("Remove from Group").clicked() {
                            let item_data = item.clone();
                            let _ = commands::items::remove_item(&mut self.config, id);
                            match item_data {
                                LaunchItem::App(a) => {
                                    commands::items::add_app(
                                        &mut self.config,
                                        a.title.clone(),
                                        a.executable_path.clone(),
                                    );
                                }
                                LaunchItem::Folder(f) => {
                                    commands::items::add_folder(
                                        &mut self.config,
                                        f.title.clone(),
                                        f.folder_path.clone(),
                                    );
                                }
                                _ => {}
                            }
                            self.mark_dirty();
                            self.context_menu = None;
                        }
                    }
                }
                ui.separator();
                let label = if is_group { "Delete Group" } else { "Remove" };
                if ui.button(label).clicked() {
                    if is_group {
                        // Check if group has items — if so, ask for confirmation
                        let has_items = self.config.items.iter().any(|i| {
                            if let LaunchItem::Group(g) = i {
                                g.id == id && !g.items.is_empty()
                            } else {
                                false
                            }
                        });
                        if has_items {
                            self.pending_delete_group = Some(id);
                        } else {
                            let _ = commands::items::delete_group(&mut self.config, id);
                            if self.nav_stack.last() == Some(&id) {
                                self.navigate_back();
                            }
                            self.mark_dirty();
                        }
                    } else {
                        let _ = commands::items::remove_item(&mut self.config, id);
                        self.mark_dirty();
                    }
                    self.context_menu = None;
                }
            });
    }
    fn render_group_selector(&mut self, ctx: &egui::Context, item_id: ItemId) {
        let groups: Vec<(ItemId, String)> = self
            .config
            .items
            .iter()
            .filter_map(|item| {
                if let LaunchItem::Group(g) = item {
                    Some((g.id, g.title.clone()))
                } else {
                    None
                }
            })
            .collect();
        egui::Window::new("Select Group")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Choose a group:");
                ui.separator();
                for (gid, name) in &groups {
                    if ui.button(name).clicked() {
                        let _ = commands::items::move_to_group(&mut self.config, item_id, *gid);
                        self.mark_dirty();
                        self.pending_group_select = None;
                    }
                }
                ui.separator();
                if ui.button("+ Create New Group").clicked() {
                    let name = unique_name(
                        "New Group",
                        &groups.iter().map(|(_, n)| n.clone()).collect::<Vec<_>>(),
                    );
                    let gid = commands::items::add_group(&mut self.config, name);
                    let _ = commands::items::move_to_group(&mut self.config, item_id, gid);
                    self.mark_dirty();
                    self.pending_group_select = None;
                }
                ui.add_space(8.0);
                if ui.button("Cancel").clicked() {
                    self.pending_group_select = None;
                }
            });
    }
}

// ─── Grid ────────────────────────────────────────────────
impl LaunchpadApp {
    fn render_grid(&mut self, ui: &mut egui::Ui, items: &[LaunchItem]) {
        let theme = self.config.resolve_theme();
        let sp = theme.grid_spacing.unwrap_or(self.config.grid_spacing);
        let isz = theme.grid_icon_size.unwrap_or(self.config.grid_icon_size);
        let iw = isz + 48.0;
        let ih = isz + 28.0;
        let tw = ui.available_width();
        let cw = iw + sp;
        let cols = ((tw + sp) / cw).floor() as usize;
        if cols == 0 {
            return;
        }
        let ox = ((tw - (cols as f32 * cw - sp)) / 2.0).max(0.0);
        for row_start in (0..items.len()).step_by(cols) {
            let row_end = (row_start + cols).min(items.len());
            let row_top = ui.cursor().top();
            let p = ui.painter().clone();
            for col in 0..(row_end - row_start) {
                let idx = row_start + col;
                let item = &items[idx];
                let x = ox + col as f32 * cw;
                let rect = egui::Rect::from_min_size(
                    egui::pos2(ui.min_rect().left() + x, row_top),
                    egui::vec2(iw, ih),
                );
                let is_sel = self.selected_index == Some(idx);
                let resp = ui.allocate_rect(rect, egui::Sense::click());
                let hovered = resp.hovered();
                let bg = if is_sel {
                    egui::Color32::from_rgba_premultiplied(0x60, 0x62, 0x68, 180)
                } else if hovered {
                    egui::Color32::from_rgba_premultiplied(0x48, 0x4A, 0x50, 120)
                } else {
                    egui::Color32::TRANSPARENT
                };
                p.rect_filled(rect, egui::CornerRadius::same(8), bg);
                if is_sel {
                    p.rect_stroke(
                        rect,
                        egui::CornerRadius::same(8),
                        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(0x80, 0x82, 0x88)),
                        egui::StrokeKind::Inside,
                    );
                }
                let icon_key = IconCache::key_for(item, isz as u32);
                if let Some(tex) = self.icon_cache.get_or_load(icon_key, ui.ctx()) {
                    let ir = egui::Rect::from_center_size(
                        rect.center() - egui::vec2(0.0, 4.0),
                        egui::vec2(isz, isz),
                    );
                    p.image(
                        tex.id(),
                        ir,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                let title = item.title();
                let display = if title.len() > 12 {
                    format!("{}…", &title[..11])
                } else {
                    title.to_string()
                };
                p.text(
                    egui::pos2(rect.center().x, rect.center().y + isz / 2.0 + 2.0),
                    egui::Align2::CENTER_TOP,
                    &display,
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_gray(220),
                );
                if resp.clicked() {
                    self.selected_index = Some(idx);
                    self.activate_item(item);
                }
                if resp.clicked_by(egui::PointerButton::Secondary) {
                    self.selected_index = Some(idx);
                    let pos = ui
                        .ctx()
                        .input(|i| i.pointer.hover_pos().unwrap_or(rect.center()));
                    self.context_menu = Some(ContextMenuState {
                        item_index: idx,
                        pos,
                    });
                }
            }
            ui.allocate_space(egui::vec2(tw, sp));
        }
    }

    /// Get mutable reference to items at the current nav level.
    fn current_items_mut(&mut self) -> &mut Vec<LaunchItem> {
        let gid = self.nav_stack.last().copied();
        if let Some(gid) = gid {
            for i in 0..self.config.items.len() {
                if let LaunchItem::Group(ref g) = self.config.items[i] {
                    if g.id == gid {
                        let ptr: *mut LaunchItem = &mut self.config.items[i];
                        return unsafe { &mut (*ptr).as_group_mut().unwrap().items };
                    }
                }
            }
        }
        &mut self.config.items
    }

    fn render_reorder_view(&mut self, ctx: &egui::Context) {
        let items: Vec<LaunchItem> = self.current_items().to_vec();
        egui::Window::new("Reorder Items")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.label(if self.is_at_root() {
                    "Reorder items — Home"
                } else {
                    "Reorder items — Group"
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        for i in 0..items.len() {
                            let item = &items[i];
                            let (icon_text, icon_color) = match item {
                                LaunchItem::App(_) => {
                                    ("A", egui::Color32::from_rgb(0x60, 0x62, 0x68))
                                }
                                LaunchItem::Group(_) => {
                                    ("G", egui::Color32::from_rgb(0xF2, 0xC9, 0x4C))
                                }
                                LaunchItem::Folder(_) => {
                                    ("F", egui::Color32::from_rgb(0x64, 0xB4, 0xFF))
                                }
                            };
                            ui.horizontal(|ui| {
                                // Draw a small colored label as icon indicator
                                let (icon_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(18.0, 18.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    icon_rect,
                                    egui::CornerRadius::same(3),
                                    icon_color,
                                );
                                ui.painter().text(
                                    icon_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    icon_text,
                                    egui::FontId::proportional(10.0),
                                    egui::Color32::WHITE,
                                );
                                ui.label(item.title());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if i > 0 {
                                            let b = egui::Button::new("");
                                            let resp = ui.add_sized(egui::vec2(24.0, 20.0), b);
                                            draw_triangle(ui, resp.rect, true);
                                            if resp.clicked() {
                                                self.swap_items(i, i - 1);
                                            }
                                        }
                                        if i < items.len() - 1 {
                                            let b = egui::Button::new("");
                                            let resp = ui.add_sized(egui::vec2(24.0, 20.0), b);
                                            draw_triangle(ui, resp.rect, false);
                                            if resp.clicked() {
                                                self.swap_items(i, i + 1);
                                            }
                                        }
                                    },
                                );
                            });
                        }
                    });
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    self.show_reorder = false;
                }
            });
    }

    fn swap_items(&mut self, a: usize, b: usize) {
        let items = self.current_items_mut();
        if a < items.len() && b < items.len() {
            items.swap(a, b);
            self.mark_dirty();
        }
    }
}

// ─── Keyboard ────────────────────────────────────────────
impl LaunchpadApp {
    fn handle_keyboard(&mut self, ctx: &Context) {
        let theme = self.config.resolve_theme();
        let isz = theme.grid_icon_size.unwrap_or(self.config.grid_icon_size);
        let sp = theme.grid_spacing.unwrap_or(self.config.grid_spacing);
        let iw = isz + 48.0;
        let cols = ((640.0_f32 + sp) / (iw + sp)).floor() as usize;
        let mut sel = self.selected_index;
        let items: Vec<LaunchItem> = self.current_items().to_vec();
        let max = items.len().saturating_sub(1);
        let inp = ctx.input(|i| i.clone());
        if inp.key_pressed(egui::Key::ArrowRight) {
            sel = Some(sel.map_or(0, |i| (i + 1).min(max)));
        }
        if inp.key_pressed(egui::Key::ArrowLeft) {
            sel = Some(sel.map_or(max, |i| i.saturating_sub(1)));
        }
        if inp.key_pressed(egui::Key::ArrowDown) {
            sel = Some(sel.map_or(0, |i| (i + cols).min(max)));
        }
        if inp.key_pressed(egui::Key::ArrowUp) {
            sel = Some(sel.map_or(0, |i| if i >= cols { i - cols } else { 0 }));
        }
        if inp.key_pressed(egui::Key::Enter) {
            if let Some(idx) = sel {
                if let Some(item) = items.get(idx) {
                    self.activate_item(item);
                }
            }
        }
        if inp.key_pressed(egui::Key::Backspace) && !self.nav_stack.is_empty() {
            self.navigate_back();
        }
        self.selected_index = sel;
    }
}

// ─── Drag & Drop ─────────────────────────────────────────
impl LaunchpadApp {
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        for file in &ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(ref path) = file.path {
                let path = std::path::PathBuf::from(path);
                if path.is_dir() {
                    let title = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Folder")
                        .to_string();
                    commands::items::add_folder(&mut self.config, title, path);
                } else {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ext == "exe" || ext == "lnk" {
                        let title = path
                            .file_stem()
                            .and_then(|n| n.to_str())
                            .unwrap_or("App")
                            .to_string();
                        commands::items::add_app(&mut self.config, title, path);
                    }
                }
                self.mark_dirty();
            }
        }
    }
}

// ─── Demo ────────────────────────────────────────────────
impl LaunchpadApp {
    fn add_demo_items(&mut self) {
        use commands::items;
        use std::path::PathBuf;
        items::add_app(
            &mut self.config,
            "Notepad".into(),
            PathBuf::from("C:\\Windows\\System32\\notepad.exe"),
        );
        items::add_app(
            &mut self.config,
            "Calculator".into(),
            PathBuf::from("C:\\Windows\\System32\\calc.exe"),
        );
        items::add_app(
            &mut self.config,
            "Command Prompt".into(),
            PathBuf::from("C:\\Windows\\System32\\cmd.exe"),
        );
        items::add_app(
            &mut self.config,
            "Paint".into(),
            PathBuf::from("C:\\Windows\\System32\\mspaint.exe"),
        );
        items::add_folder(
            &mut self.config,
            "Documents".into(),
            PathBuf::from("C:\\Users\\X4Lo\\Documents"),
        );
        items::add_folder(
            &mut self.config,
            "Downloads".into(),
            PathBuf::from("C:\\Users\\X4Lo\\Downloads"),
        );
        let g = items::add_group(&mut self.config, "Games".into());
        items::add_app_to_group(
            &mut self.config,
            g,
            "Steam".into(),
            PathBuf::from("C:\\Program Files (x86)\\Steam\\steam.exe"),
        )
        .ok();
        let w = items::add_group(&mut self.config, "Work".into());
        items::add_app_to_group(
            &mut self.config,
            w,
            "Notepad".into(),
            PathBuf::from("C:\\Windows\\System32\\notepad.exe"),
        )
        .ok();
        self.mark_dirty();
    }
}

fn group_name(config: &Config, gid: ItemId) -> String {
    config
        .items
        .iter()
        .filter_map(|item| {
            if let LaunchItem::Group(g) = item {
                if g.id == gid {
                    Some(g.title.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .next()
        .unwrap_or_default()
}

fn group_name_with_icon(config: &Config, gid: ItemId) -> (String, egui::Color32) {
    config
        .items
        .iter()
        .filter_map(|item| match item {
            LaunchItem::Group(g) if g.id == gid => {
                Some((g.title.clone(), egui::Color32::from_rgb(0xF2, 0xC9, 0x4C)))
            }
            LaunchItem::Folder(f) if f.id == gid => {
                Some((f.title.clone(), egui::Color32::from_rgb(0x64, 0xB4, 0xFF)))
            }
            _ => None,
        })
        .next()
        .unwrap_or_default()
}
fn unique_name(base: &str, existing: &[String]) -> String {
    if !existing.iter().any(|n| n == base) {
        return base.to_string();
    }
    for i in 1..100 {
        let c = format!("{} {}", base, i);
        if !existing.iter().any(|n| n == &c) {
            return c;
        }
    }
    format!("{} {}", base, existing.len() + 1)
}

fn swatch(ui: &mut egui::Ui, label: &str, hex: &str) {
    if let Some(color) = crate::config::manager::Theme::parse_hex(hex) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 18.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(3), color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            egui::Color32::BLACK,
        );
    }
}

fn hex_opt(s: &Option<String>) -> Option<egui::Color32> {
    crate::config::manager::Theme::parse_hex(s.as_deref()?)
}

// ─── Icon drawing helpers (no emojis) ─────────────────────

/// Draw up and down arrow triangles for the reorder toggle button.
fn draw_updown_arrows(p: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let cx = rect.center().x;
    let gap = 2.0;
    let hw = 4.0; // half-width of triangle
                  // Up arrow
    let up_y = rect.center().y - gap;
    p.line_segment(
        [egui::pos2(cx, up_y - 3.0), egui::pos2(cx, up_y + 1.0)],
        egui::Stroke::new(1.5_f32, color),
    );
    p.line_segment(
        [egui::pos2(cx - hw, up_y - 1.0), egui::pos2(cx, up_y + 1.0)],
        egui::Stroke::new(1.5_f32, color),
    );
    p.line_segment(
        [egui::pos2(cx + hw, up_y - 1.0), egui::pos2(cx, up_y + 1.0)],
        egui::Stroke::new(1.5_f32, color),
    );
    // Down arrow
    let dn_y = rect.center().y + gap;
    p.line_segment(
        [egui::pos2(cx, dn_y - 1.0), egui::pos2(cx, dn_y + 3.0)],
        egui::Stroke::new(1.5_f32, color),
    );
    p.line_segment(
        [egui::pos2(cx - hw, dn_y + 1.0), egui::pos2(cx, dn_y - 1.0)],
        egui::Stroke::new(1.5_f32, color),
    );
    p.line_segment(
        [egui::pos2(cx + hw, dn_y + 1.0), egui::pos2(cx, dn_y - 1.0)],
        egui::Stroke::new(1.5_f32, color),
    );
}

/// Draw a simple cog/gear icon using a circle and a dot.
fn draw_cog(p: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let c = rect.center();
    let r = 5.0;
    // Outer circle
    p.circle_stroke(c, r + 1.0, egui::Stroke::new(1.5_f32, color));
    // Inner dot
    p.circle_filled(c, 2.0, color);
}

/// Draw an up-pointing or down-pointing triangle.
fn draw_triangle(ui: &egui::Ui, rect: egui::Rect, up: bool) {
    let p = ui.painter();
    let c = rect.center();
    let color = egui::Color32::from_gray(180);
    let hw = 5.0;
    if up {
        let top = c.y - 3.0;
        let bot = c.y + 4.0;
        p.line_segment(
            [egui::pos2(c.x - hw, bot), egui::pos2(c.x, top)],
            egui::Stroke::new(1.5_f32, color),
        );
        p.line_segment(
            [egui::pos2(c.x + hw, bot), egui::pos2(c.x, top)],
            egui::Stroke::new(1.5_f32, color),
        );
        p.line_segment(
            [egui::pos2(c.x - hw, bot), egui::pos2(c.x + hw, bot)],
            egui::Stroke::new(1.5_f32, color),
        );
    } else {
        let top = c.y - 4.0;
        let bot = c.y + 3.0;
        p.line_segment(
            [egui::pos2(c.x - hw, top), egui::pos2(c.x, bot)],
            egui::Stroke::new(1.5_f32, color),
        );
        p.line_segment(
            [egui::pos2(c.x + hw, top), egui::pos2(c.x, bot)],
            egui::Stroke::new(1.5_f32, color),
        );
        p.line_segment(
            [egui::pos2(c.x - hw, top), egui::pos2(c.x + hw, top)],
            egui::Stroke::new(1.5_f32, color),
        );
    }
}
