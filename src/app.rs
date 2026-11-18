//! Application controller. Lives on the main (Slint event-loop) thread and glues the
//! UI, config, hotkey, tray and the icon worker together. Callbacks reach it through
//! `with_app(|a| ...)`.
//!
//! The launcher window is created once and only hidden/shown, so the hotkey brings it
//! up instantly. The Settings window is built on demand and destroyed when closed.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use slint::winit_030::winit::event::{ElementState, MouseButton, WindowEvent};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::commands::items::{self, Parent};
use crate::config::manager::{expand_path, Config, ConfigManager, RESERVED_THEME_NAMES};
use crate::models::item::{ItemId, LaunchItem};
use crate::platform::autostart;
use crate::platform::hotkey::{self, capture_from_key_event, Capture, Hotkey};
use crate::platform::shell::{self, IconSource, Rgba};
use crate::platform::tray::{Tray, TrayCommand};
use crate::ui::{self, theme};
use crate::{GroupChoice, MainWindow, SettingsWindow, ThemeColors, ThemeOption, Tile};

const SAVE_DELAY: Duration = Duration::from_millis(600);
const STATUS_FOR: Duration = Duration::from_secs(5);
const ERROR_FOR: Duration = Duration::from_secs(8);
const CAPTURE_HINT: &str = "Press the new combination · Esc cancels · Backspace clears";

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Run `f` with the app, if it exists. Safe to call from any event-loop callback.
pub fn with_app<R>(f: impl FnOnce(&Rc<App>) -> R) -> Option<R> {
    let app = APP.with(|a| a.borrow().clone());
    app.map(|a| f(&a))
}

/// Run `f` on the UI thread from any thread.
fn on_ui(f: impl FnOnce(&Rc<App>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        with_app(f);
    });
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Dialog {
    None,
    Rename(ItemId),
    NewGroup,
    Move(ItemId),
    DeleteGroup(ItemId),
    FirstRun,
    Migrate,
}

#[derive(Clone, Copy, PartialEq)]
enum StatusAction {
    None,
    Undo,
}

pub struct App {
    cfg: RefCell<Config>,
    store: RefCell<ConfigManager>,
    window: MainWindow,
    settings: RefCell<Option<SettingsWindow>>,

    tiles: Rc<VecModel<Tile>>,
    group_choices: Rc<VecModel<GroupChoice>>,
    theme_options: Rc<VecModel<ThemeOption>>,

    nav: Cell<Parent>,
    /// The group "Back" just left, for the mouse's Forward button.
    forward: Cell<Parent>,
    query: RefCell<String>,
    dialog: Cell<Dialog>,
    migration: RefCell<Vec<(ItemId, PathBuf)>>,
    /// Item tree before the last removal, for "Undo".
    undo: RefCell<Option<Vec<LaunchItem>>>,
    status_action: Cell<StatusAction>,

    icons: RefCell<HashMap<IconSource, Option<slint::Image>>>,
    pending_icons: RefCell<HashSet<IconSource>>,
    icon_tx: mpsc::Sender<IconSource>,
    missing: RefCell<HashMap<PathBuf, bool>>,

    hotkey: RefCell<Option<Hotkey>>,
    hotkey_error: RefCell<String>,
    capturing: Cell<bool>,
    /// Theme editor: `None` closed, `Some(None)` creating, `Some(Some(name))` editing.
    editing_theme: RefCell<Option<Option<String>>>,
    tray: RefCell<Option<Tray>>,

    dropped: RefCell<Vec<PathBuf>>,
    geometry_restored: Cell<bool>,
    geometry_dirty: Cell<bool>,
    refresh_pending: Cell<bool>,
    save_timer: slint::Timer,
    status_timer: slint::Timer,
    started: std::time::Instant,
}

impl App {
    pub fn start(start_hidden: bool) -> Result<Rc<App>, String> {
        let store = ConfigManager::new().map_err(|e| e.to_string())?;
        let (cfg, load_error) = store.load();
        log::info!("Config: {} ({} items)", store.path().display(), cfg.items.len());

        let window = MainWindow::new().map_err(|e| e.to_string())?;
        let icon_tx = shell::spawn_icon_worker(|source, icon| on_ui(move |a| a.icon_ready(source, icon)));

        let app = Rc::new(App {
            cfg: RefCell::new(cfg),
            store: RefCell::new(store),
            window,
            settings: RefCell::new(None),
            tiles: Rc::new(VecModel::default()),
            group_choices: Rc::new(VecModel::default()),
            theme_options: Rc::new(VecModel::default()),
            nav: Cell::new(None),
            forward: Cell::new(None),
            query: RefCell::default(),
            dialog: Cell::new(Dialog::None),
            migration: RefCell::default(),
            undo: RefCell::new(None),
            status_action: Cell::new(StatusAction::None),
            icons: RefCell::default(),
            pending_icons: RefCell::default(),
            icon_tx,
            missing: RefCell::default(),
            hotkey: RefCell::new(None),
            hotkey_error: RefCell::default(),
            capturing: Cell::new(false),
            editing_theme: RefCell::new(None),
            tray: RefCell::new(None),
            dropped: RefCell::default(),
            geometry_restored: Cell::new(false),
            geometry_dirty: Cell::new(false),
            refresh_pending: Cell::new(false),
            save_timer: slint::Timer::default(),
            status_timer: slint::Timer::default(),
            started: std::time::Instant::now(),
        });
        APP.with(|a| *a.borrow_mut() = Some(app.clone()));

        app.wire_main_window();
        app.apply_theme();
        app.push_layout();

        match Hotkey::new(|| on_ui(|a| a.toggle_window())) {
            Ok(h) => *app.hotkey.borrow_mut() = Some(h),
            Err(e) => log::error!("global hotkey unavailable: {e}"),
        }
        app.apply_hotkey();

        // The tray must be created once the event loop runs.
        slint::Timer::single_shot(Duration::ZERO, || {
            with_app(|a| a.init_tray());
        });

        let first_run = app.store.borrow().pending_first_run;
        if first_run {
            app.open_dialog(
                Dialog::FirstRun,
                "Welcome to Launchpad",
                "Where should Launchpad keep its settings? You can't change this later without moving config.json.",
                "",
            );
        } else {
            app.check_icon_migration();
        }

        if let Some(e) = load_error {
            app.set_status(&e, true, StatusAction::None);
        }
        app.refresh();
        if !start_hidden || first_run {
            app.show_window();
        }
        Ok(app)
    }

    /// Tear down in a safe order.
    pub fn shutdown() {
        let Some(app) = APP.with(|a| a.borrow_mut().take()) else { return };
        app.remember_geometry();
        app.save_now();
        app.settings.borrow_mut().take();
        app.tray.borrow_mut().take();
        app.hotkey.borrow_mut().take();
        let _ = app.window.hide();
    }

    // ── Wiring ─────────────────────────────────────────────────────────────────

    fn wire_main_window(&self) {
        let w = &self.window;
        w.set_app_icon(ui::app_icon_image(64));
        w.set_tiles(ModelRc::from(self.tiles.clone()));
        w.set_group_choices(ModelRc::from(self.group_choices.clone()));

        w.window().on_close_requested(|| {
            with_app(|a| {
                if a.cfg.borrow().close_to_tray && a.tray.borrow().is_some() {
                    a.hide_window();
                } else {
                    a.quit();
                }
            });
            slint::CloseRequestResponse::HideWindow
        });

        w.window().on_winit_window_event(|_, event| {
            match event {
                WindowEvent::HoveredFile(_) => on_ui(|a| a.set_files_hovering(true)),
                WindowEvent::HoveredFileCancelled => on_ui(|a| a.set_files_hovering(false)),
                // Mouse side buttons navigate like a browser.
                WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Back, .. } => {
                    on_ui(|a| a.nav_back());
                }
                WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Forward, .. } => {
                    on_ui(|a| a.nav_forward());
                }
                WindowEvent::DroppedFile(path) => {
                    let path = path.clone();
                    on_ui(move |a| a.file_dropped(path));
                }
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    // Read the geometry later (never borrow app state inside winit's dispatch).
                    on_ui(|a| {
                        if a.geometry_restored.get() {
                            a.geometry_dirty.set(true);
                            a.schedule_save();
                        }
                    });
                }
                _ => {}
            }
            EventResult::Propagate
        });

        w.on_activate(|id| {
            with_app(|a| a.activate(&id));
        });
        w.on_search_changed(|q| {
            with_app(|a| a.search_changed(q.as_str()));
        });
        w.on_go_back(|| {
            with_app(|a| a.go_back());
        });
        w.on_hide_requested(|| {
            with_app(|a| a.hide_window());
        });
        w.on_open_settings(|| {
            with_app(|a| a.open_settings());
        });
        w.on_add_app(|| {
            with_app(|a| a.pick_paths(false));
        });
        w.on_add_folder(|| {
            with_app(|a| a.pick_paths(true));
        });
        w.on_new_group(|| {
            with_app(|a| {
                let name = items::unique_group_name(&a.cfg.borrow(), "New Group");
                a.open_dialog(Dialog::NewGroup, "New group", "Drag items onto a group to add them.", &name);
            });
        });
        w.on_menu_action(|action, id| {
            with_app(|a| a.menu_action(action.as_str(), &id));
        });
        w.on_drop_tile(|id, to, into| {
            with_app(|a| a.drop_tile(&id, to, into));
        });
        w.on_dialog_confirm(|text| {
            with_app(|a| a.dialog_confirm(text.as_str()));
        });
        w.on_dialog_cancel(|| {
            with_app(|a| a.dialog_cancel());
        });
        w.on_dialog_alt(|| {
            with_app(|a| a.dialog_alt());
        });
        w.on_move_to(|gid| {
            with_app(|a| a.move_to(&gid));
        });
        w.on_first_run_choice(|portable| {
            with_app(|a| a.first_run_choice(portable));
        });
        w.on_status_clicked(|| {
            with_app(|a| a.status_clicked());
        });
        w.on_dismiss_status(|| {
            with_app(|a| a.clear_status());
        });
        w.on_fit_icon_size(|width, height, count| {
            let gap = with_app(|a| a.cfg.borrow().grid_spacing).unwrap_or(12.0);
            ui::fit_icon_size(width, height, count.max(0) as usize, gap)
        });
    }

    fn init_tray(&self) {
        let label = hotkey::label(&self.cfg.borrow().hotkey);
        match Tray::new(&label, |cmd| {
            with_app(|a| match cmd {
                TrayCommand::Toggle => a.toggle_window(),
                TrayCommand::Settings => a.open_settings(),
                TrayCommand::Quit => a.quit(),
            });
        }) {
            Ok(t) => *self.tray.borrow_mut() = Some(t),
            Err(e) => {
                log::error!("tray icon unavailable: {e}");
                self.show_window();
            }
        }
    }

    // ── Window lifecycle ───────────────────────────────────────────────────────

    pub fn show_window(&self) {
        self.reload_if_changed();
        let w = self.window.window();
        if !self.geometry_restored.get() {
            let cfg = self.cfg.borrow();
            if let (Some(width), Some(height)) = (cfg.window_width, cfg.window_height) {
                if width >= 300.0 && height >= 240.0 {
                    w.set_size(slint::LogicalSize::new(width, height));
                }
            }
            if let (Some(x), Some(y)) = (cfg.window_x, cfg.window_y) {
                w.set_position(slint::LogicalPosition::new(x, y));
            }
        }
        // Re-check missing targets each time the launcher opens.
        self.missing.borrow_mut().clear();
        self.refresh();

        if let Err(e) = self.window.show() {
            log::error!("cannot show window: {e}");
            return;
        }
        let dark = theme::current(&self.cfg.borrow()).dark;
        let first = !self.geometry_restored.get();
        w.with_winit_window(|ww| {
            ui::apply_native_chrome(ww, dark);
            if first {
                keep_on_screen(ww);
            }
            ww.set_minimized(false);
            ww.focus_window();
        });
        self.geometry_restored.set(true);
        self.window.invoke_focus_search();
        self.apply_chrome_soon();
    }

    /// A window that's shown for the first time has no native window until the event
    /// loop runs, so title-bar theme / icons are applied again on the next tick.
    fn apply_chrome_soon(&self) {
        slint::Timer::single_shot(Duration::from_millis(30), || {
            with_app(|a| {
                let dark = theme::current(&a.cfg.borrow()).dark;
                a.window.window().with_winit_window(|ww| ui::apply_native_chrome(ww, dark));
                if let Some(s) = a.settings.borrow().as_ref() {
                    s.window().with_winit_window(|ww| ui::apply_native_chrome(ww, dark));
                }
            });
        });
    }

    fn hide_window(&self) {
        self.remember_geometry();
        self.schedule_save();
        let _ = self.window.hide();
    }

    fn toggle_window(&self) {
        let w = self.window.window();
        if !w.is_visible() {
            self.show_window();
            return;
        }
        let state = w.with_winit_window(|ww| (ww.has_focus(), ww.is_minimized().unwrap_or(false)));
        match state {
            Some((true, false)) => self.hide_window(),
            _ => self.show_window(),
        }
    }

    fn quit(&self) {
        self.remember_geometry();
        self.save_now();
        let _ = slint::quit_event_loop();
    }

    /// Copy the window's logical geometry into the config.
    fn remember_geometry(&self) {
        let w = self.window.window();
        if !self.geometry_restored.get() || !w.is_visible() {
            return;
        }
        if w.with_winit_window(|ww| ww.is_minimized().unwrap_or(false)).unwrap_or(false) {
            return;
        }
        let scale = w.scale_factor();
        let pos = w.position().to_logical(scale);
        let size = w.size().to_logical(scale);
        let mut cfg = self.cfg.borrow_mut();
        cfg.window_x = Some(pos.x);
        cfg.window_y = Some(pos.y);
        cfg.window_width = Some(size.width);
        cfg.window_height = Some(size.height);
        self.geometry_dirty.set(false);
    }

    // ── Persistence ────────────────────────────────────────────────────────────

    fn schedule_save(&self) {
        self.save_timer.start(slint::TimerMode::SingleShot, SAVE_DELAY, || {
            with_app(|a| {
                if a.geometry_dirty.get() {
                    a.remember_geometry();
                }
                a.save_now();
            });
        });
    }

    fn save_now(&self) {
        self.save_timer.stop();
        // Edited by hand while we were running: take the file's version rather than
        // overwriting it (only window geometry or a just-made change is lost).
        if self.reload_if_changed() {
            return;
        }
        if let Err(e) = self.store.borrow().save(&self.cfg.borrow()) {
            log::error!("save: {e}");
            self.set_status(&format!("Couldn't save settings: {e}"), true, StatusAction::None);
        }
    }

    /// Re-read config.json if something else changed it. Returns true if it did (or
    /// if the new file doesn't parse, in which case we keep ours and say so).
    fn reload_if_changed(&self) -> bool {
        if !self.store.borrow().changed_externally() {
            return false;
        }
        let result = self.store.borrow().try_load();
        match result {
            Ok(cfg) => {
                log::info!("config.json changed on disk; reloading");
                *self.cfg.borrow_mut() = cfg;
                self.icons.borrow_mut().retain(|k, _| !matches!(k, IconSource::File(_)));
                self.apply_theme();
                self.push_layout();
                self.apply_hotkey();
                self.refresh();
                self.set_status("Reloaded config.json (it was edited outside Launchpad)", false, StatusAction::None);
            }
            Err(e) => {
                self.set_status(&format!("{e} — fix it and Launchpad will pick it up."), true, StatusAction::None)
            }
        }
        true
    }

    /// Mutate the config, then save and redraw.
    fn edit(&self, f: impl FnOnce(&mut Config)) {
        f(&mut self.cfg.borrow_mut());
        self.schedule_save();
        self.refresh();
    }

    // ── Content ────────────────────────────────────────────────────────────────

    fn schedule_refresh(&self) {
        if self.refresh_pending.replace(true) {
            return;
        }
        slint::Timer::single_shot(Duration::from_millis(16), || {
            with_app(|a| {
                a.refresh_pending.set(false);
                a.refresh();
            });
        });
    }

    /// Rebuild the tiles for the current group / search.
    fn refresh(&self) {
        let cfg = self.cfg.borrow();
        if let Some(gid) = self.nav.get() {
            if items::find_group(&cfg, gid).is_none() {
                self.nav.set(None);
            }
        }
        let nav = self.nav.get();
        let query = self.query.borrow().clone();
        let searching = !query.trim().is_empty();

        let entries: Vec<(LaunchItem, Parent)> = if searching {
            items::search(&cfg, &query)
        } else {
            items::list(&cfg, nav).map(|l| l.iter().map(|i| (i.clone(), nav)).collect()).unwrap_or_default()
        };
        let tiles: Vec<Tile> =
            entries.iter().map(|(item, parent)| self.make_tile(&cfg, item, *parent, searching)).collect();

        let w = &self.window;
        let count = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
        if searching {
            w.set_heading("Search".into());
            w.set_subheading(count(tiles.len(), "result", "results").into());
        } else if let Some(g) = nav.and_then(|gid| items::find_group(&cfg, gid)) {
            w.set_heading(g.title.as_str().into());
            w.set_subheading(format!("Group · {}", count(g.items.len(), "item", "items")).into());
        } else {
            w.set_heading("Launchpad".into());
            w.set_subheading(if tiles.is_empty() {
                SharedString::new()
            } else {
                count(tiles.len(), "item", "items").into()
            });
        }
        w.set_in_group(nav.is_some() && !searching);
        let target = nav.and_then(|gid| items::find_group(&cfg, gid)).map_or("Home".to_owned(), |g| g.title.clone());
        w.set_drop_target_name(target.into());
        drop(cfg);

        // Update rows in place when the count is unchanged, so an icon arriving
        // mid-drag doesn't rebuild (and drop) the tile under the pointer.
        if self.tiles.row_count() == tiles.len() {
            for (i, t) in tiles.into_iter().enumerate() {
                if self.tiles.row_data(i).as_ref() != Some(&t) {
                    self.tiles.set_row_data(i, t);
                }
            }
        } else {
            self.tiles.set_vec(tiles);
        }
        let sel = w.get_selected();
        if sel >= self.tiles.row_count() as i32 {
            w.set_selected(self.tiles.row_count() as i32 - 1);
        }
    }

    fn make_tile(&self, cfg: &Config, item: &LaunchItem, parent: Parent, searching: bool) -> Tile {
        let title = item.title();
        let kind = match item {
            LaunchItem::App(_) => 0,
            LaunchItem::Group(_) => 1,
            LaunchItem::Folder(_) => 2,
        };
        let target = target_of(item);
        let icon = self.icon_for(item);
        let (previews, count) = match item {
            LaunchItem::Group(g) => {
                let imgs: Vec<slint::Image> = g.items.iter().filter_map(|i| self.icon_for(i)).take(4).collect();
                (ModelRc::new(VecModel::from(imgs)), g.items.len() as i32)
            }
            _ => (ModelRc::default(), 0),
        };
        let location = match (searching, parent) {
            (true, Some(gid)) => items::find_group(cfg, gid).map(|g| g.title.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        Tile {
            id: item.id().to_string().into(),
            kind,
            title: title.into(),
            location: location.into(),
            in_group: parent.is_some(),
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
            custom_icon: items::icon_path(item).is_some(),
            initial: ui::initial_for(title).into(),
            tint: ui::tint_for(title),
            previews,
            count,
            missing: target.is_some_and(|p| self.is_missing(&p)),
        }
    }

    fn is_missing(&self, path: &Path) -> bool {
        *self.missing.borrow_mut().entry(path.to_path_buf()).or_insert_with(|| !path.exists())
    }

    // ── Icons ──────────────────────────────────────────────────────────────────

    /// The item's icon if it's loaded; otherwise queue it (a refresh follows) and
    /// return `None` so the tile shows its letter for now.
    fn icon_for(&self, item: &LaunchItem) -> Option<slint::Image> {
        let custom = items::icon_path(item).map(|p| IconSource::File(self.store.borrow().resolve_icon(p)));
        let shell = target_of(item).map(IconSource::Shell);
        for source in custom.into_iter().chain(shell) {
            match self.icons.borrow().get(&source) {
                Some(Some(img)) => return Some(img.clone()),
                Some(None) => continue, // failed: fall back to the next source
                None => {}
            }
            if self.pending_icons.borrow_mut().insert(source.clone()) {
                let _ = self.icon_tx.send(source);
            }
            return None;
        }
        None
    }

    fn icon_ready(&self, source: IconSource, icon: Option<Rgba>) {
        let done = {
            let mut pending = self.pending_icons.borrow_mut();
            pending.remove(&source);
            pending.is_empty()
        };
        if done {
            log::debug!("icons loaded ({} cached) after {:?}", self.icons.borrow().len() + 1, self.started.elapsed());
        }
        let img = icon.map(|i| ui::image_from_rgba(i.width, i.height, &i.pixels));
        self.icons.borrow_mut().insert(source, img);
        self.schedule_refresh();
    }

    fn forget_icon(&self, source: &IconSource) {
        self.icons.borrow_mut().remove(source);
    }

    // ── Navigation & activation ────────────────────────────────────────────────

    fn activate(&self, id: &str) {
        let Some(id) = parse_id(id) else { return };
        let item = items::find(&self.cfg.borrow(), id).map(|(i, _)| i.clone());
        let Some(item) = item else { return };
        let target = match &item {
            LaunchItem::Group(g) => {
                self.enter_group(g.id);
                return;
            }
            LaunchItem::App(_) | LaunchItem::Folder(_) => target_of(&item).unwrap_or_default(),
        };
        shell::open(target, |msg| on_ui(move |a| a.set_status(&msg, true, StatusAction::None)));
        if self.cfg.borrow().hide_on_launch {
            self.clear_search();
            self.hide_window();
        }
    }

    fn enter_group(&self, gid: ItemId) {
        self.forward.set(None);
        self.nav.set(Some(gid));
        self.clear_search();
        self.window.set_selected(-1);
        self.refresh();
    }

    fn go_back(&self) {
        let from = self.nav.take();
        self.forward.set(from);
        self.refresh();
        // Select the group we just left, so Enter goes straight back in.
        let idx = from.and_then(|gid| self.cfg.borrow().items.iter().position(|i| i.id() == gid));
        self.window.set_selected(idx.map_or(-1, |i| i as i32));
    }

    /// Mouse Back: clear a search first, then leave the group.
    fn nav_back(&self) {
        if self.dialog.get() != Dialog::None {
            return;
        }
        if !self.query.borrow().trim().is_empty() {
            self.clear_search();
            self.search_changed("");
        } else if self.nav.get().is_some() {
            self.go_back();
        }
    }

    /// Mouse Forward: back into the group that Back just left.
    fn nav_forward(&self) {
        if self.dialog.get() != Dialog::None || self.nav.get().is_some() {
            return;
        }
        if let Some(gid) = self.forward.get() {
            if items::find_group(&self.cfg.borrow(), gid).is_some() {
                self.enter_group(gid);
            }
        }
    }

    fn clear_search(&self) {
        self.query.borrow_mut().clear();
        self.window.set_query(SharedString::new());
    }

    fn search_changed(&self, q: &str) {
        *self.query.borrow_mut() = q.to_owned();
        self.refresh();
        let has_results = self.tiles.row_count() > 0;
        self.window.set_selected(if !q.trim().is_empty() && has_results { 0 } else { -1 });
    }

    // ── Item actions ───────────────────────────────────────────────────────────

    fn menu_action(&self, action: &str, id: &str) {
        let Some(id) = parse_id(id) else { return };
        let found = items::find(&self.cfg.borrow(), id).map(|(i, p)| (i.clone(), p));
        let Some((item, parent)) = found else { return };
        match action {
            "open" => self.activate(&id.to_string()),
            "reveal" => {
                if let Some(p) = target_of(&item) {
                    shell::reveal(&p);
                }
            }
            "rename" => self.open_dialog(Dialog::Rename(id), "Rename", "", item.title()),
            "icon" => self.pick_icon(id),
            "reset-icon" => {
                if let Some(p) = items::icon_path(&item) {
                    self.forget_icon(&IconSource::File(self.store.borrow().resolve_icon(p)));
                }
                self.edit(|c| {
                    let _ = items::set_icon(c, id, None);
                });
            }
            "move" => {
                if matches!(item, LaunchItem::Group(_)) {
                    return;
                }
                let choices: Vec<GroupChoice> = {
                    let cfg = self.cfg.borrow();
                    items::groups(&cfg)
                        .into_iter()
                        .map(|(gid, title)| GroupChoice {
                            id: gid.to_string().into(),
                            title: title.into(),
                            count: items::find_group(&cfg, gid).map_or(0, |g| g.items.len() as i32),
                            current: parent == Some(gid),
                        })
                        .collect()
                };
                self.group_choices.set_vec(choices);
                self.open_dialog(Dialog::Move(id), &format!("Move “{}” to a group", item.title()), "", "");
            }
            "move-out" => {
                self.edit(|c| {
                    let _ = items::move_to_root(c, id);
                });
                self.set_status(&format!("Moved {} to Home", item.title()), false, StatusAction::None);
            }
            "ungroup" => self.ungroup(id),
            "remove" => match &item {
                LaunchItem::Group(g) if !g.items.is_empty() => {
                    let n = g.items.len();
                    let msg = format!(
                        "It holds {} — they'll be removed from Launchpad too. Choose Ungroup to keep them.",
                        if n == 1 { "1 item".to_owned() } else { format!("{n} items") }
                    );
                    self.open_dialog(Dialog::DeleteGroup(id), &format!("Delete “{}”?", g.title), &msg, "");
                }
                _ => self.remove_with_undo(id, item.title()),
            },
            _ => log::warn!("unknown menu action {action}"),
        }
    }

    fn remove_with_undo(&self, id: ItemId, title: &str) {
        let snapshot = self.cfg.borrow().items.clone();
        self.edit(|c| {
            let _ = items::remove_item(c, id);
        });
        *self.undo.borrow_mut() = Some(snapshot);
        self.set_status(&format!("Removed {title}"), false, StatusAction::Undo);
    }

    fn ungroup(&self, gid: ItemId) {
        if self.nav.get() == Some(gid) {
            self.nav.set(None);
        }
        self.edit(|c| {
            let _ = items::ungroup(c, gid);
        });
    }

    /// A tile was dragged within the grid.
    fn drop_tile(&self, id: &str, to: i32, into: bool) {
        let Some(id) = parse_id(id) else { return };
        if !self.query.borrow().trim().is_empty() || to < 0 {
            return;
        }
        let nav = self.nav.get();
        let (from, target) = {
            let cfg = self.cfg.borrow();
            let Some(list) = items::list(&cfg, nav) else { return };
            let Some(from) = list.iter().position(|i| i.id() == id) else { return };
            (from, list.get(to as usize).cloned())
        };
        if into {
            if let Some(LaunchItem::Group(g)) = target {
                let mut result = Ok(());
                self.edit(|c| result = items::move_to_group(c, id, g.id));
                match result {
                    Ok(()) => self.set_status(&format!("Moved to {}", g.title), false, StatusAction::None),
                    Err(e) => self.set_status(&e, true, StatusAction::None),
                }
                self.window.set_selected(-1);
            }
            return;
        }
        self.edit(|c| {
            let _ = items::reorder(c, nav, from, to as usize);
        });
        self.window.set_selected(to);
    }

    // ── Adding ─────────────────────────────────────────────────────────────────

    fn set_files_hovering(&self, on: bool) {
        self.window.set_files_hovering(on);
    }

    /// Explorer delivers one event per dropped file; collect them, then add in one go.
    fn file_dropped(&self, path: PathBuf) {
        self.window.set_files_hovering(false);
        let first = {
            let mut d = self.dropped.borrow_mut();
            d.push(path);
            d.len() == 1
        };
        if first {
            slint::Timer::single_shot(Duration::from_millis(30), || {
                with_app(|a| {
                    let paths = std::mem::take(&mut *a.dropped.borrow_mut());
                    a.add_paths(paths);
                });
            });
        }
    }

    fn pick_paths(&self, folders: bool) {
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new();
            let picked = if folders {
                dialog.set_title("Add folders to Launchpad").pick_folders()
            } else {
                dialog
                    .set_title("Add apps to Launchpad")
                    .add_filter("Apps and shortcuts", &["exe", "lnk", "url", "appref-ms", "bat", "cmd", "msc"])
                    .add_filter("All files", &["*"])
                    .pick_files()
            };
            if let Some(paths) = picked {
                on_ui(move |a| a.add_paths(paths));
            }
        });
    }

    fn add_paths(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let nav = self.nav.get();
        let mut added = 0;
        self.edit(|c| {
            for p in paths {
                if items::add_path(c, nav, p).is_some() {
                    added += 1;
                }
            }
        });
        let target = nav.and_then(|g| items::find_group(&self.cfg.borrow(), g).map(|g| g.title.clone()));
        let target = target.unwrap_or_else(|| "Home".into());
        match added {
            0 => self.set_status("Nothing was added — those items couldn't be found.", true, StatusAction::None),
            1 => self.set_status(&format!("Added 1 item to {target}"), false, StatusAction::None),
            n => self.set_status(&format!("Added {n} items to {target}"), false, StatusAction::None),
        }
    }

    fn pick_icon(&self, id: ItemId) {
        let icons_dir = self.store.borrow().icons_dir();
        std::thread::spawn(move || {
            let Some(src) = rfd::FileDialog::new()
                .set_title("Choose an icon")
                .add_filter("Images", &["png", "ico", "jpg", "jpeg", "bmp"])
                .pick_file()
            else {
                return;
            };
            // Copy into the icons folder so the config stays portable.
            let _ = std::fs::create_dir_all(&icons_dir);
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
            let name = PathBuf::from(format!("{}.{ext}", uuid::Uuid::new_v4()));
            let result = std::fs::copy(&src, icons_dir.join(&name)).map_err(|e| e.to_string());
            on_ui(move |a| match result {
                Ok(_) => a.edit(|c| {
                    let _ = items::set_icon(c, id, Some(name));
                }),
                Err(e) => a.set_status(&format!("Couldn't copy the icon: {e}"), true, StatusAction::None),
            });
        });
    }

    // ── Dialogs ────────────────────────────────────────────────────────────────

    fn open_dialog(&self, d: Dialog, title: &str, message: &str, text: &str) {
        let kind = match d {
            Dialog::None => "",
            Dialog::Rename(_) => "rename",
            Dialog::NewGroup => "new-group",
            Dialog::Move(_) => "move",
            Dialog::DeleteGroup(_) => "delete-group",
            Dialog::FirstRun => "first-run",
            Dialog::Migrate => "migrate",
        };
        self.dialog.set(d);
        let w = &self.window;
        w.set_dialog_title(title.into());
        w.set_dialog_message(message.into());
        w.set_dialog_text(text.into());
        w.set_dialog(kind.into());
    }

    fn close_dialog(&self) {
        self.dialog.set(Dialog::None);
        self.window.set_dialog(SharedString::new());
    }

    fn dialog_confirm(&self, text: &str) {
        let name = text.trim().to_owned();
        match self.dialog.get() {
            Dialog::Rename(id) => {
                if name.is_empty() {
                    return;
                }
                self.edit(|c| {
                    let _ = items::rename_item(c, id, name);
                });
            }
            Dialog::NewGroup => {
                if name.is_empty() {
                    return;
                }
                self.edit(|c| {
                    items::add_group(c, name);
                });
            }
            Dialog::Move(id) => {
                if name.is_empty() {
                    return;
                }
                let mut result = Ok(());
                self.edit(|c| {
                    let gid = items::add_group(c, name.clone());
                    result = items::move_to_group(c, id, gid);
                });
                match result {
                    Ok(()) => {
                        self.set_status(&format!("Created {name} and moved the item there"), false, StatusAction::None)
                    }
                    Err(e) => self.set_status(&e, true, StatusAction::None),
                }
            }
            Dialog::DeleteGroup(gid) => {
                let title = items::find_group(&self.cfg.borrow(), gid).map(|g| g.title.clone()).unwrap_or_default();
                self.remove_with_undo(gid, &title);
            }
            Dialog::Migrate => self.run_icon_migration(),
            Dialog::FirstRun | Dialog::None => {}
        }
        self.close_dialog();
    }

    fn dialog_cancel(&self) {
        match self.dialog.get() {
            Dialog::FirstRun => return,
            Dialog::Migrate => self.migration.borrow_mut().clear(),
            _ => {}
        }
        self.close_dialog();
    }

    fn dialog_alt(&self) {
        if let Dialog::DeleteGroup(gid) = self.dialog.get() {
            self.ungroup(gid);
        }
        self.close_dialog();
    }

    fn move_to(&self, gid: &str) {
        let (Dialog::Move(id), Some(gid)) = (self.dialog.get(), parse_id(gid)) else { return };
        let mut result = Ok(());
        self.edit(|c| result = items::move_to_group(c, id, gid));
        let name = items::find_group(&self.cfg.borrow(), gid).map(|g| g.title.clone()).unwrap_or_default();
        match result {
            Ok(()) => self.set_status(&format!("Moved to {name}"), false, StatusAction::None),
            Err(e) => self.set_status(&e, true, StatusAction::None),
        }
        self.close_dialog();
    }

    fn first_run_choice(&self, portable: bool) {
        let result = {
            let cfg = self.cfg.borrow();
            self.store.borrow_mut().finalize_first_run(portable, &cfg)
        };
        self.close_dialog();
        match result {
            Ok(is_portable) => {
                if portable && !is_portable {
                    self.set_status(
                        "Couldn't write next to launchpad.exe, so settings are kept in AppData instead.",
                        true,
                        StatusAction::None,
                    );
                }
                self.save_now();
                self.push_settings();
            }
            Err(e) => self.set_status(&format!("Couldn't create the settings file: {e}"), true, StatusAction::None),
        }
    }

    /// Older versions stored absolute icon paths; offer to copy them into icons/.
    fn check_icon_migration(&self) {
        let icons_dir = self.store.borrow().icons_dir();
        let found = find_external_icons(&self.cfg.borrow(), &icons_dir);
        if found.is_empty() {
            return;
        }
        let n = found.len();
        *self.migration.borrow_mut() = found;
        self.open_dialog(
            Dialog::Migrate,
            "Copy custom icons?",
            &format!(
                "{} point to images outside Launchpad's icons folder. Copy them in so they keep working if the originals move?",
                if n == 1 { "1 custom icon".to_owned() } else { format!("{n} custom icons") }
            ),
            "",
        );
    }

    fn run_icon_migration(&self) {
        let icons_dir = self.store.borrow().icons_dir();
        let _ = std::fs::create_dir_all(&icons_dir);
        let todo = std::mem::take(&mut *self.migration.borrow_mut());
        let mut copied = 0;
        self.edit(|c| {
            for (id, old) in todo {
                let ext = old.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
                let name = PathBuf::from(format!("{}.{ext}", uuid::Uuid::new_v4()));
                if std::fs::copy(&old, icons_dir.join(&name)).is_ok() && items::set_icon(c, id, Some(name)).is_ok() {
                    copied += 1;
                }
            }
        });
        self.set_status(&format!("Copied {copied} icon(s) into the icons folder"), false, StatusAction::None);
    }

    // ── Status banner ──────────────────────────────────────────────────────────

    fn set_status(&self, msg: &str, error: bool, action: StatusAction) {
        if action != StatusAction::Undo {
            self.undo.borrow_mut().take();
        }
        self.status_action.set(action);
        let w = &self.window;
        w.set_status(msg.into());
        w.set_status_is_error(error);
        w.set_status_action(if action == StatusAction::Undo { "Undo".into() } else { SharedString::new() });
        self.status_timer.start(slint::TimerMode::SingleShot, if error { ERROR_FOR } else { STATUS_FOR }, || {
            with_app(|a| a.clear_status());
        });
    }

    fn clear_status(&self) {
        self.status_timer.stop();
        self.undo.borrow_mut().take();
        self.status_action.set(StatusAction::None);
        self.window.set_status(SharedString::new());
        self.window.set_status_action(SharedString::new());
    }

    fn status_clicked(&self) {
        if self.status_action.get() == StatusAction::Undo {
            if let Some(items) = self.undo.borrow_mut().take() {
                self.edit(|c| c.items = items);
            }
        }
        self.clear_status();
    }

    // ── Theme & layout ─────────────────────────────────────────────────────────

    fn apply_theme(&self) {
        let colors = theme::current(&self.cfg.borrow());
        apply_colors(&self.window, &colors);
        let w = self.window.window();
        if w.is_visible() {
            w.with_winit_window(|ww| ui::apply_native_chrome(ww, colors.dark));
        }
        if let Some(s) = self.settings.borrow().as_ref() {
            apply_settings_colors(s, &colors);
            s.window().with_winit_window(|ww| ui::apply_native_chrome(ww, colors.dark));
        }
    }

    fn push_layout(&self) {
        let cfg = self.cfg.borrow();
        let w = &self.window;
        w.set_base_icon_size(cfg.grid_icon_size);
        w.set_spacing(cfg.grid_spacing);
        w.set_auto_fit(cfg.auto_fit);
        w.set_keep_on_top(cfg.always_on_top);
    }

    // ── Hotkey ─────────────────────────────────────────────────────────────────

    fn apply_hotkey(&self) {
        let (text, on_release) = {
            let c = self.cfg.borrow();
            (c.hotkey.clone(), c.hotkey_on_release)
        };
        let result = match self.hotkey.borrow_mut().as_mut() {
            Some(h) => h.apply(&text, on_release),
            None => Err("Global shortcuts aren't available on this system.".into()),
        };
        *self.hotkey_error.borrow_mut() = result.err().unwrap_or_default();
        if let Some(t) = self.tray.borrow().as_ref() {
            t.set_hotkey_label(&hotkey::label(&text));
        }
        self.push_settings();
    }

    fn start_capture(&self) {
        if let Some(h) = self.hotkey.borrow_mut().as_mut() {
            h.suspend();
        }
        self.capturing.set(true);
        self.push_settings();
    }

    fn end_capture(&self) {
        if self.capturing.replace(false) {
            self.apply_hotkey();
        }
    }

    fn key_captured(&self, text: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) {
        match capture_from_key_event(text, ctrl, alt, shift, meta) {
            Capture::Captured(acc) => {
                self.cfg.borrow_mut().hotkey = acc.to_string();
                self.schedule_save();
                self.end_capture();
            }
            Capture::Clear => {
                self.cfg.borrow_mut().hotkey.clear();
                self.schedule_save();
                self.end_capture();
            }
            Capture::Cancel => self.end_capture(),
            Capture::Pending => {}
            Capture::Invalid(why) => {
                if let Some(s) = self.settings.borrow().as_ref() {
                    s.set_hotkey_note(why.into());
                    s.set_hotkey_note_is_error(true);
                }
            }
        }
    }

    // ── Settings window ────────────────────────────────────────────────────────

    fn open_settings(&self) {
        self.reload_if_changed();
        if self.settings.borrow().is_none() {
            match self.build_settings() {
                Ok(s) => *self.settings.borrow_mut() = Some(s),
                Err(e) => {
                    log::error!("cannot create settings window: {e}");
                    return;
                }
            }
        }
        self.push_settings();
        let dark = theme::current(&self.cfg.borrow()).dark;
        if let Some(s) = self.settings.borrow().as_ref() {
            let _ = s.show();
            s.window().with_winit_window(|ww| {
                ui::apply_native_chrome(ww, dark);
                ww.set_minimized(false);
                ww.focus_window();
            });
        }
        self.apply_chrome_soon();
    }

    fn build_settings(&self) -> Result<SettingsWindow, slint::PlatformError> {
        let s = SettingsWindow::new()?;
        s.set_app_icon(ui::app_icon_image(64));
        s.set_version(env!("CARGO_PKG_VERSION").into());
        s.set_themes(ModelRc::from(self.theme_options.clone()));
        apply_settings_colors(&s, &theme::current(&self.cfg.borrow()));

        s.window().on_close_requested(|| {
            // Can't drop the component inside its own callback; do it next tick.
            slint::Timer::single_shot(Duration::ZERO, || {
                with_app(|a| {
                    a.end_capture();
                    a.editing_theme.borrow_mut().take();
                    a.settings.borrow_mut().take();
                });
            });
            slint::CloseRequestResponse::HideWindow
        });
        s.on_start_capture(|| {
            with_app(|a| a.start_capture());
        });
        s.on_cancel_capture(|| {
            with_app(|a| a.end_capture());
        });
        s.on_key_captured(|text, ctrl, alt, shift, meta| {
            with_app(|a| a.key_captured(text.as_str(), ctrl, alt, shift, meta));
        });
        s.on_clear_hotkey(|| {
            with_app(|a| {
                a.cfg.borrow_mut().hotkey.clear();
                a.schedule_save();
                a.capturing.set(false);
                a.apply_hotkey();
            });
        });
        s.on_set_flag(|name, v| {
            with_app(|a| a.set_flag(name.as_str(), v));
        });
        s.on_pick_theme(|i| {
            with_app(|a| {
                let names = theme::names(&a.cfg.borrow());
                if let Some(name) = names.get(i as usize) {
                    a.cfg.borrow_mut().selected_theme = (name != "Dark").then(|| name.clone());
                    a.schedule_save();
                    a.apply_theme();
                    a.push_settings();
                }
            });
        });
        s.on_set_icon_size(|v| {
            with_app(|a| {
                a.cfg.borrow_mut().grid_icon_size = v.clamp(32.0, 128.0);
                a.schedule_save();
                a.push_layout();
                a.push_settings();
            });
        });
        s.on_set_spacing(|v| {
            with_app(|a| {
                a.cfg.borrow_mut().grid_spacing = v.clamp(0.0, 40.0);
                a.schedule_save();
                a.push_layout();
                a.push_settings();
            });
        });
        s.on_open_url(|url| shell::open_url(url.as_str()));
        s.on_hex_color(|text| theme::parse_color(text.as_str()).unwrap_or(slint::Color::from_argb_u8(0, 0, 0, 0)));
        s.on_hex_valid(|text| theme::parse_color(text.as_str()).is_some());
        s.on_create_theme(|| {
            with_app(|a| a.open_theme_editor(None));
        });
        s.on_edit_theme(|i| {
            with_app(|a| {
                let names = theme::names(&a.cfg.borrow());
                if let Some(name) = names.get(i as usize).cloned() {
                    a.open_theme_editor(Some(name));
                }
            });
        });
        s.on_save_theme(|| {
            with_app(|a| a.save_theme());
        });
        s.on_cancel_theme(|| {
            with_app(|a| a.close_theme_editor());
        });
        s.on_delete_theme(|| {
            with_app(|a| a.delete_theme());
        });
        s.on_open_config_dir(|| {
            with_app(|a| {
                if let Some(dir) = a.store.borrow().path().parent() {
                    shell::open_folder(dir);
                }
            });
        });
        Ok(s)
    }

    // ── Theme editor ───────────────────────────────────────────────────────────

    /// `None` = create a new theme starting from the current one's colours.
    fn open_theme_editor(&self, editing: Option<String>) {
        let settings = self.settings.borrow();
        let Some(s) = settings.as_ref() else { return };
        let cfg = self.cfg.borrow();
        let (name, base) = match &editing {
            Some(name) => (name.clone(), name.clone()),
            None => {
                let taken = theme::names(&cfg);
                let free = |n: &String| !taken.iter().any(|t| t.eq_ignore_ascii_case(n));
                let name = std::iter::once("My theme".to_owned())
                    .chain((2..).map(|i| format!("My theme {i}")))
                    .find(free)
                    .unwrap();
                (name, theme::current_name(&cfg))
            }
        };
        let [bg, surface, hover, accent, border, text] = theme::editable_colors(&cfg, &base);
        s.set_ed_name(name.into());
        s.set_ed_bg(bg.into());
        s.set_ed_surface(surface.into());
        s.set_ed_hover(hover.into());
        s.set_ed_accent(accent.into());
        s.set_ed_border(border.into());
        s.set_ed_text(text.into());
        s.set_editor_error(SharedString::new());
        s.set_editor_new(editing.is_none());
        s.set_editor_open(true);
        *self.editing_theme.borrow_mut() = Some(editing);
    }

    fn close_theme_editor(&self) {
        self.editing_theme.borrow_mut().take();
        if let Some(s) = self.settings.borrow().as_ref() {
            s.set_editor_open(false);
        }
    }

    fn save_theme(&self) {
        let Some(original) = self.editing_theme.borrow().clone() else { return };
        let (name, colors) = {
            let settings = self.settings.borrow();
            let Some(s) = settings.as_ref() else { return };
            let colors = [
                s.get_ed_bg(),
                s.get_ed_surface(),
                s.get_ed_hover(),
                s.get_ed_accent(),
                s.get_ed_border(),
                s.get_ed_text(),
            ]
            .map(|c| c.trim().to_string());
            (s.get_ed_name().trim().to_string(), colors)
        };
        let error = {
            let cfg = self.cfg.borrow();
            let clash = |n: &str| {
                RESERVED_THEME_NAMES.iter().any(|r| r.eq_ignore_ascii_case(n))
                    || cfg.themes.iter().any(|t| {
                        t.name.eq_ignore_ascii_case(n) && original.as_deref().is_none_or(|o| !o.eq_ignore_ascii_case(n))
                    })
            };
            if name.is_empty() {
                Some("Give the theme a name.".to_owned())
            } else if clash(&name) {
                Some(format!("There's already a theme called “{name}”."))
            } else if colors.iter().any(|c| theme::parse_color(c).is_none()) {
                Some("Every colour needs a 6-digit hex value, like #1f2127.".to_owned())
            } else {
                None
            }
        };
        if let Some(e) = error {
            if let Some(s) = self.settings.borrow().as_ref() {
                s.set_editor_error(e.into());
            }
            return;
        }

        let theme = theme::custom_theme(&name, &colors);
        {
            let mut cfg = self.cfg.borrow_mut();
            let pos = original.as_deref().and_then(|o| cfg.themes.iter().position(|t| t.name.eq_ignore_ascii_case(o)));
            match pos {
                Some(i) => cfg.themes[i] = theme,
                None => cfg.themes.push(theme),
            }
            cfg.selected_theme = Some(name);
        }
        self.close_theme_editor();
        self.schedule_save();
        self.apply_theme();
        self.push_settings();
    }

    fn delete_theme(&self) {
        let Some(Some(original)) = self.editing_theme.borrow().clone() else { return };
        {
            let mut cfg = self.cfg.borrow_mut();
            cfg.themes.retain(|t| !t.name.eq_ignore_ascii_case(&original));
            if cfg.selected_theme.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(&original)) {
                cfg.selected_theme = None;
            }
        }
        self.close_theme_editor();
        self.schedule_save();
        self.apply_theme();
        self.push_settings();
    }

    fn set_flag(&self, name: &str, v: bool) {
        match name {
            "hotkey-on-release" => {
                self.cfg.borrow_mut().hotkey_on_release = v;
                self.apply_hotkey();
            }
            "hide-on-launch" => self.cfg.borrow_mut().hide_on_launch = v,
            "keep-on-top" => self.cfg.borrow_mut().always_on_top = v,
            "close-to-tray" => self.cfg.borrow_mut().close_to_tray = v,
            "auto-fit" => self.cfg.borrow_mut().auto_fit = v,
            "auto-start" => match autostart::set_enabled(v) {
                Ok(()) => self.cfg.borrow_mut().auto_start = v,
                Err(e) => {
                    self.set_status(&format!("Couldn't change Start with Windows: {e}"), true, StatusAction::None)
                }
            },
            _ => log::warn!("unknown setting {name}"),
        }
        self.schedule_save();
        self.push_layout();
        self.push_settings();
    }

    fn push_settings(&self) {
        let settings = self.settings.borrow();
        let Some(s) = settings.as_ref() else { return };
        let cfg = self.cfg.borrow();
        s.set_dark(theme::current(&cfg).dark);
        s.set_hotkey(if cfg.hotkey.is_empty() { SharedString::new() } else { hotkey::label(&cfg.hotkey).into() });
        s.set_capturing(self.capturing.get());
        let err = self.hotkey_error.borrow();
        let (note, is_err) = if self.capturing.get() {
            (CAPTURE_HINT.to_owned(), false)
        } else if !err.is_empty() {
            (err.clone(), true)
        } else {
            (String::new(), false)
        };
        s.set_hotkey_note(note.into());
        s.set_hotkey_note_is_error(is_err);
        s.set_hotkey_on_release(cfg.hotkey_on_release);
        s.set_hide_on_launch(cfg.hide_on_launch);
        s.set_keep_on_top(cfg.always_on_top);
        s.set_close_to_tray(cfg.close_to_tray);
        s.set_auto_start(cfg.auto_start);
        s.set_auto_fit(cfg.auto_fit);
        s.set_icon_size(cfg.grid_icon_size);
        s.set_spacing(cfg.grid_spacing);

        let options = theme::options(&cfg);
        if self.theme_options.iter().ne(options.iter().cloned()) {
            self.theme_options.set_vec(options);
        }
        let current = theme::current_name(&cfg);
        let idx = theme::names(&cfg).iter().position(|n| *n == current).unwrap_or(0);
        s.set_theme_index(idx as i32);

        let store = self.store.borrow();
        s.set_config_path(store.path().display().to_string().into());
        let portable = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf))
            == store.path().parent().map(Path::to_path_buf);
        s.set_portable(portable);
    }
}

fn apply_colors(w: &MainWindow, colors: &ThemeColors) {
    w.global::<crate::Theme>().set_c(colors.clone());
    w.set_dark(colors.dark);
}

fn apply_settings_colors(s: &SettingsWindow, colors: &ThemeColors) {
    s.global::<crate::Theme>().set_c(colors.clone());
    s.set_dark(colors.dark);
}

/// The file or folder an item opens, with `$(Drive)` expanded. `None` for groups.
fn target_of(item: &LaunchItem) -> Option<PathBuf> {
    match item {
        LaunchItem::App(a) => Some(expand_path(&a.executable_path)),
        LaunchItem::Folder(f) => Some(expand_path(&f.folder_path)),
        LaunchItem::Group(_) => None,
    }
}

fn parse_id(s: &str) -> Option<ItemId> {
    uuid::Uuid::parse_str(s).ok().map(ItemId)
}

/// Move the window to the primary monitor if its saved position is off every screen
/// (e.g. a monitor was unplugged).
fn keep_on_screen(ww: &slint::winit_030::winit::window::Window) {
    let Ok(pos) = ww.outer_position() else { return };
    let size = ww.outer_size();
    let (cx, cy) = (pos.x + size.width as i32 / 2, pos.y + 20);
    let visible = ww.available_monitors().any(|m| {
        let (mp, ms) = (m.position(), m.size());
        cx >= mp.x && cx < mp.x + ms.width as i32 && cy >= mp.y && cy < mp.y + ms.height as i32
    });
    if visible {
        return;
    }
    if let Some(m) = ww.primary_monitor().or_else(|| ww.available_monitors().next()) {
        let (mp, ms) = (m.position(), m.size());
        let x = mp.x + (ms.width as i32 - size.width as i32).max(0) / 2;
        let y = mp.y + (ms.height as i32 - size.height as i32).max(0) / 3;
        ww.set_outer_position(slint::winit_030::winit::dpi::PhysicalPosition::new(x, y));
    }
}

/// Custom icon paths pointing outside the icons directory (old config format).
fn find_external_icons(config: &Config, icons_dir: &Path) -> Vec<(ItemId, PathBuf)> {
    fn scan(items: &[LaunchItem], icons_dir: &Path, out: &mut Vec<(ItemId, PathBuf)>) {
        for item in items {
            if let Some(p) = items::icon_path(item) {
                // Bare filenames are already in the icons folder.
                let external = p.parent().is_some_and(|parent| !parent.as_os_str().is_empty() && parent != icons_dir);
                if external && p.exists() {
                    out.push((item.id(), p.clone()));
                }
            }
            if let LaunchItem::Group(g) = item {
                scan(&g.items, icons_dir, out);
            }
        }
    }
    let mut out = Vec::new();
    scan(&config.items, icons_dir, &mut out);
    out
}
