# Launchpad — Technical Reference

> Covers every module, struct, enum, trait, and public function.

---

## Architecture Overview

```
main.rs
 ├── Creates ConfigManager, loads Config
 ├── Creates crossbeam channels (hotkey, tray)
 ├── Spawns system tray (tray_icon + TrayIconBuilder)
 ├── Registers global hotkey Ctrl+Alt+R
 ├── Builds LaunchpadApp with all runtime handles
 └── Runs eframe::run_native

Event Flow:

  Hotkey thread                Tray menu callback
       │                              │
       ▼                              ▼
  hotkey_tx ──► channel ──► hotkey_rx      tray_tx ◄── channel ◄── set_event_handler
                                 │              │
                                 ▼              ▼
                          LaunchpadApp::update() reads both channels each frame
                                 │
                                 ▼
                          ctx.send_viewport_cmd(Focus)   or   Quit
```

### Module Dependency Graph

```mermaid
graph TD
    main --> app
    main --> config
    main --> platform
    main --> utils
    app --> commands
    app --> config
    app --> models
    app --> platform
    app --> ui
    config --> models
    models --> [serde/uuid]
    commands --> config
    commands --> models
    platform --> [global-hotkey/tray-icon/crossbeam]
    ui --> models
    ui --> config
```

---

## Module: `main.rs`

**Path:** `src/main.rs`

### Entry Point

```rust
fn main() -> Result<(), Box<dyn std::error::Error>>
```

Initialization order:

1. **Logging** — `env_logger` configured with default filter `"info"`.
2. **Config** — `ConfigManager::new()` creates the `%APPDATA%/Launchpad/` directory; `load()` reads or seeds `config.json`.
3. **Channels** — Two unbounded crossbeam channels:
   - `(hotkey_tx, hotkey_rx): Sender/Receiver<()>` — hotkey toggle signals.
   - `(tray_tx, tray_rx): Sender/Receiver<TrayEvent>` — tray menu events.
4. **System Tray** — `generate_tray_icon()` + `tray::create_tray()` build and display the tray icon with a "Show / Hide" + "Quit" menu.
5. **Global Hotkey** — `hotkey::register_hotkey()` registers `Ctrl+Alt+R`, spawns a listener thread. The returned `GlobalHotKeyManager` is `mem::forget`-ed so it stays alive for the process lifetime.
6. **Window Config** — `eframe::NativeOptions`:
   - Frameless (`with_decorations(false)`)
   - Always on top
   - No taskbar entry
   - Resizable
   - Default inner size 640×480
7. **App** — `LaunchpadApp::new(hotkey_rx, tray_rx, config, config_manager)`.
8. **Run** — `eframe::run_native(...)` starts the event loop.

### Inner Attributes

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

Suppresses the console window in release builds on Windows.

---

## Module: `app.rs`

**Path:** `src/app.rs`

### `LaunchpadApp`

The central application state struct.

```rust
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
}
```

Key fields:

| Field | Type | Purpose |
|---|---|---|
| `pending_rename` | `Option<(ItemId, String)>` | When set, shows a rename dialog modal. |
| `pending_group_select` | `Option<ItemId>` | When set, shows the "Select Group" picker for moving an item. |
| `pending_delete_group` | `Option<ItemId>` | When set, shows a confirmation dialog before deleting a non-empty group. |
| `resizing` / `resize_start` | `Option<ResizeEdge>` / `Option<Pos2>` | Tracks active window resize drag. Auto-resets if the mouse button is released to prevent stuck state. |

| Field | Type | Description |
|---|---|---|
| `hotkey_rx` | `Receiver<()>` | Receives toggle signals from the global hotkey thread. |
| `tray_rx` | `Receiver<TrayEvent>` | Receives Toggle/Quit events from the tray menu. |
| `config` | `Config` | In-memory configuration (items, layout, window state). |
| `config_manager` | `ConfigManager` | Handle for persisting config to disk. |
| `dirty` | `bool` | Set `true` when config changes need saving. |
| `nav_stack` | `Vec<ItemId>` | Stack of group IDs the user has navigated into. Empty = root. |
| `selected_index` | `Option<usize>` | Currently selected item index within the current view. |
| `icon_cache` | `IconCache` | LRU-like cache mapping `IconKey` → `TextureHandle`. |
| `show_settings` | `bool` | Whether the Settings window is visible. |
| `context_menu` | `Option<ContextMenuState>` | Active context menu, if any. |
| `pos_restored` | `bool` | Guard ensuring window position/size restore runs only once. |

### `ContextMenuState`

```rust
struct ContextMenuState {
    item_index: usize,   // index into current_items()
    pos: egui::Pos2,     // fixed screen position for the menu window
}
```

Cloneable. Captured at right-click time so the menu renders at a stable position.

### App Lifecycle

#### `pub fn new(...)` — Constructor

Accepts the two channel receivers, `Config`, and `ConfigManager`. Initializes all fields to defaults (`dirty: false`, empty nav stack, `selected_index: None`, `show_settings: false`, `context_menu: None`, `pos_restored: false`).

#### `eframe::App::update(&mut self, ctx, _frame)` — Main loop

Called every frame. Order of operations:

1. **Store context** — `CTX` thread-local is updated for use by non-`update` methods.
2. **Drain channels** — Hotkey → `Focus`; TrayEvent::Toggle → `Focus`; TrayEvent::Quit → save & exit.
3. **Restore window position** — On first frame only, send `OuterPosition` and `InnerSize` viewport commands from saved config.
4. **Track window state** — Read `outer_rect` and `inner_rect` from viewport input each frame. Only mark dirty if values actually changed.
5. **Save** — `save_if_dirty()` auto-persists changes (including position/size).
6. **Apply theme** — `ui::theme::apply_theme(ctx, &self.config)` applies the selected theme from config.
7. **Title bar** — `egui::TopBottomPanel::top("title_bar")` with theme-colored frame renders drag handle, settings gear, close button.
8. **Modals** — Settings window, rename dialog, group selector, delete confirmation (if pending).
9. **Context menu** — If `context_menu` is `Some`, render the anchored menu.
10. **Central panel** — Breadcrumb nav bar ("Launchpad" + `▸` + group names with icons), custom divider, then empty state or item grid.
11. **Resize handles** — Overlay Areas at the window edges.
12. **Keyboard handling** — Arrow keys, Enter, Backspace.
13. **Request repaint** — Continuous repaint for smooth interaction.

### Internal Helpers

| Method | Signature | Description |
|---|---|---|
| `mark_dirty` | `fn(&mut self)` | Sets `dirty = true`. |
| `save_if_dirty` | `fn(&mut self)` | If dirty, calls `config_manager.save()` and clears the flag. |
| `current_items` | `fn(&self) -> &[LaunchItem]` | Returns items at the current nav level (root or inside a group). |
| `is_at_root` | `fn(&self) -> bool` | `nav_stack.is_empty()`. |
| `navigate_to_root` | `fn(&mut self)` | Clears nav stack and selection. |
| `navigate_into` | `fn(&mut self, gid: ItemId)` | Pushes a group onto the nav stack. |
| `navigate_back` | `fn(&mut self)` | Pops the nav stack. |
| `activate_item` | `fn(&mut self, item: &LaunchItem)` | Dispatches: spawns process for apps, opens Explorer for folders, navigates into groups. Optionally minimizes if `hide_on_launch`. |

### Rendering Methods

#### `render_title_bar(&mut self, ui, line_color)`

- 32px tall drag handle (theme `header_color` background from panel frame).
- Title text "Launchpad" centered.
- Settings gear (⚙) toggles `show_settings`.
- Close button as two drawn line segments forming an X, red on hover.
- Bottom accent line in `line_color` (theme `divider_color`).

#### `render_settings(&mut self, ui)`

Settings window with two collapsible sections:
- **General** — Grid Spacing slider (2–24 px), Icon Size slider (24–72 px), Hide on Launch checkbox.
- **Themes** — Lists all themes as selectable labels. Selecting one sets `selected_theme` and marks dirty. Shows color swatch previews for the active theme.

#### `render_context_menu(&mut self, ctx, state)`

Rendered as an `egui::Window` with no title bar, fixed position, and 150px default width. Buttons: Open/Open Folder, Rename, Change Icon, Remove Icon (if custom), Add to Group (apps/folders), Remove from Group (when inside a group), Delete Group/Remove. Non-empty groups show a confirmation dialog before deletion.

#### `render_grid(&mut self, ui, items)`

Custom grid using theme-aware spacing and icon size. Computes column count from available width, centers horizontally. Each cell:
- Background fill (transparent / hover gray / selected gray with border).
- Icon from `IconCache` centered above the label.
- Truncated title text (max 12 chars + ellipsis).
- Left-click activates the item.
- Right-click opens context menu at cursor position.

#### `render_resize_handles(&mut self, ctx)`

Three 6px overlay Areas at window edges for custom resize (right, bottom, corner). Each is a separate `egui::Area` so they stay reachable at any window size. Auto-resets stuck resize state if no mouse button is held.

#### `handle_keyboard(&mut self, ctx)`

- Arrow keys move `selected_index` (left/right by 1, up/down by `columns`).
- Enter activates the selected item.
- Backspace navigates back if not at root.
- Column count uses theme-aware spacing and icon size.

### `add_demo_items(&mut self)`

Populates empty config with hardcoded demo items: Chrome, VS Code, Terminal, Notepad, Documents (folder), Downloads (folder), Games group (Steam), Work group (Excel). Calls `commands::items::*` functions and marks dirty.

### `thread_local` CTX Pattern

```rust
thread_local! {
    static CTX: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
}
fn ctx() -> egui::Context { CTX.with(|c| c.borrow().clone().unwrap()) }
```

Stores the egui context in a thread-local so that non-`update` methods (like `activate_item`) can send viewport commands (e.g., `Minimized`). The context is refreshed every frame at the top of `update`.

### `group_name(config, gid) -> String`

Free function. Looks up a group's title by ID in the config. Returns empty string if not found (rendered as, e.g., an unlabeled breadcrumb).

---

## Module: `config/manager.rs`

**Path:** `src/config/manager.rs`

### Configuration System Overview

The configuration system is built around a single `Config` struct that holds all user-facing settings. On disk, settings are persisted as JSON in a file named `config.json`.

#### Config file resolution

`ConfigManager::new()` resolves the config path through a four-step decision chain:

1. **Existing portable** — If `config.json` already exists next to the executable, use it (no prompt).
2. **Existing AppData** — If `%APPDATA%/Launchpad/config.json` already exists, use it (no prompt).
3. **First-run dialog** — If no config exists anywhere, a native dialog asks the user to choose:
   - **Yes = Portable mode**: config stored next to the exe (ideal for USB drives).
   - **No = Normal mode**: config stored in `%APPDATA%/Launchpad/` (recommended for installed apps).
   - If portable is chosen but the exe directory is read-only (e.g. `Program Files`), a warning appears and the app falls back to AppData.
4. **Fallback** — AppData mode (either chosen by user, or as fallback from failed portable).

| Mode | Location | When used |
|---|---|---|
| Portable | `<exe_dir>/config.json` | Already exists, or user chose it on first run |
| AppData | `%APPDATA%/Launchpad/config.json` | Default, or user chose it, or portable fallback |

#### First-startup seeding

When the config path is determined and no file exists yet, a default config is seeded — either during `new()` (portable mode) or `load()` (AppData mode). The seed is `Config::default()` serialized as pretty-printed JSON, containing every setting key with its default value. This ensures:

- All keys are present from the start (no "missing key" surprises later)
- Users can inspect and hand-edit the file with full context
- Serde's `#[serde(default)]` on each field handles forward compatibility when new keys are added in future versions

#### How serde defaults work

Each field in `Config` is annotated with `#[serde(default)]` (or `#[serde(default = "fn_name")]` for non-standard defaults). When deserializing, serde substitutes the field's default value if the key is absent from JSON. This means:

- **New keys added in updates** don't break existing config files — they simply get the default value
- **Removed keys** are silently ignored by serde (no error)
- `Config::default()` is kept in sync with serde defaults, providing a canonical source of truth

#### Atomic saves

`ConfigManager::save()` writes config atomically: serialize to JSON, write to a `.tmp` file, then rename over the real path. This protects against corruption from crashes or power loss mid-write.

#### Auto-start sync

On load, `ConfigManager::load()` cross-checks the `auto_start` flag against the actual Windows registry state. If they differ (e.g., the user manually toggled it via Task Manager), the config is corrected to match reality and saved.

### Adding a new setting key — step-by-step

When you need to add a new configuration option, follow this checklist:

1. **Add the field** to the `Config` struct in `src/config/manager.rs` with `#[serde(default)]` (or `#[serde(default = "fn_name")]` for non-bool/non-zero defaults).
2. **Add the field** to `Config::default()` with the same default value.
3. **Add UI** in `LaunchpadApp::render_settings()` (in `src/app.rs`) so the user can change it.
4. **Wire up behavior** wherever the setting takes effect (e.g., in `update()`, `render_title_bar()`, etc.).
5. **Call `self.mark_dirty()`** whenever the setting changes so it gets persisted.

> The first-startup seeding in `ConfigManager::load()` means the new key will automatically appear in the JSON file the next time a fresh config is created. Existing configs will pick up the default via `#[serde(default)]` on next load.

### `Config`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub items: Vec<LaunchItem>,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub window_x: Option<f32>,
    pub window_y: Option<f32>,
    pub grid_spacing: f32,          // default 12.0
    pub grid_icon_size: f32,        // default 48.0
    pub hide_on_launch: bool,       // default false
    pub close_to_tray: bool,        // default false
    pub themes: Vec<Theme>,         // built-in + user themes
    pub selected_theme: Option<String>, // name of active theme
    pub hotkey: String,             // default "Ctrl+Alt+R"
    pub hotkey_on_release: bool,    // default true
    pub auto_start: bool,           // default false
}
```

All fields use `#[serde(default)]` (or a custom default function). `grid_spacing` defaults to `12.0`, `grid_icon_size` to `48.0`, and `themes` defaults to `Theme::builtin_themes()`.

`Config::default()` mirrors the serde defaults explicitly.

**Persistence format:** JSON with pretty-printing. The `LaunchItem` enum uses internal tagging (`"type": "app" | "group" | "folder"`).

#### Window geometry persistence

Window position and size are saved **immediately** on every change — not just on app close. The `update()` method compares current viewport rect against stored values and calls `mark_dirty()` whenever they differ, triggering a save on the next frame.

#### Portable mode

If `config.json` exists in the same directory as the executable, `ConfigManager::new()` uses it directly — no files are written to `%APPDATA%`. This enables fully portable deployments: drop the exe and config in a folder and run.

### Theme System

See [Themes](themes.md) for full documentation.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub header_color: Option<String>,      // hex, e.g. "313337"
    pub body_color: Option<String>,
    pub widget_color: Option<String>,
    pub selection_color: Option<String>,
    pub divider_color: Option<String>,
    pub text_color: Option<String>,
    pub corner_radius: Option<u8>,
    pub grid_spacing: Option<f32>,
    pub grid_icon_size: Option<f32>,
}
```

Key methods:

| Method | Description |
|---|---|
| `Theme::default_theme()` | Returns the built-in "Default" dark theme. |
| `Theme::builtin_themes()` | Returns Default, Dracula, Nord, and Catppuccin themes. |
| `Theme::parse_hex("313337")` | Parses a 6-char hex string (with or without `#`) into `egui::Color32`. |
| `Config::resolve_theme()` | Looks up `selected_theme` by name (case-insensitive) in the themes list. Missing fields fall back to the Default theme. If the theme name isn't found, returns Default. |

On every load, `ConfigManager::load()` merges built-in themes into the user's config so new themes appear automatically after updates.

### `ConfigManager`

```rust
pub struct ConfigManager {
    config_path: PathBuf,
}
```

Stores the path to `%APPDATA%/Launchpad/config.json` (or platform equivalent via `dirs::config_dir()`).

| Method | Signature | Description |
|---|---|---|
| `new` | `fn() -> Result<Self>` | Creates the `Launchpad` config directory and builds the manager. |
| `load` | `fn(&self) -> Result<Config>` | Reads and deserializes config. If the file doesn't exist, seeds with `Config::default()` and saves. On parse error, falls back to default with a warning. |
| `save` | `fn(&self, config: &Config) -> Result<()>` | Writes config atomically: serializes to JSON, writes to a `.tmp` file, then renames it over the real path. |

**Atomic writes** protect against corruption from crashes mid-write. The `tmp → rename` pattern is a common POSIX/Windows-safe strategy.

---

## Module: `models/`

**Path:** `src/models/`

### `ItemId` (`models/item.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ItemId(pub Uuid);
```

A newtype around `uuid::Uuid` (v4). Provides stable identity across renames and moves.

| Method | Description |
|---|---|
| `new() -> Self` | Generates a new UUID v4. |
| `Display` impl | Formats as the inner UUID string. |

### `LaunchItem` (`models/item.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LaunchItem {
    #[serde(rename = "app")]    App(AppItem),
    #[serde(rename = "group")]  Group(GroupItem),
    #[serde(rename = "folder")] Folder(FolderItem),
}
```

Internally tagged enum. The JSON discriminator field is `"type"` with values `"app"`, `"group"`, or `"folder"`.

**Example JSON:**

```json
{
  "type": "app",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Chrome",
  "executable_path": "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"
}
```

| Method | Returns | Description |
|---|---|---|
| `title()` | `&str` | Delegates to the inner variant's `title` field. |
| `id()` | `ItemId` | Delegates to the inner variant's `id` field. |

### `AppItem` (`models/app_item.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppItem {
    pub id: ItemId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<PathBuf>,
    pub executable_path: PathBuf,
}
```

| Constructor | Description |
|---|---|
| `AppItem::new(title, executable_path)` | Creates with a fresh `ItemId` and no custom icon. |

### `GroupItem` (`models/group_item.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupItem {
    pub id: ItemId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<PathBuf>,
    #[serde(default)]
    pub items: Vec<LaunchItem>,
}
```

Groups are intentionally flat — they contain `LaunchItem` values but the navigation model (`nav_stack`) only goes one level deep. Groups cannot be nested inside other groups in the tree.

| Constructor | Description |
|---|---|
| `GroupItem::new(title)` | Creates with a fresh `ItemId`, no custom icon, empty items vec. |

### `FolderItem` (`models/folder_item.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderItem {
    pub id: ItemId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<PathBuf>,
    pub folder_path: PathBuf,
}
```

Represents a shortcut to a directory. When activated, opens the folder in Windows Explorer via `explorer <folder_path>`.

| Constructor | Description |
|---|---|
| `FolderItem::new(title, folder_path)` | Creates with a fresh `ItemId` and no custom icon. |

### Re-exports (`models/mod.rs`)

```rust
pub use app_item::AppItem;
pub use folder_item::FolderItem;
pub use group_item::GroupItem;
pub use item::{ItemId, LaunchItem};
```

All four types are publicly available at the `crate::models` level.

---

## Module: `commands/items.rs`

**Path:** `src/commands/items.rs`

All command functions operate on `&mut Config` and return either an `ItemId` (for creation) or a `Result<(), String>` (for mutations that can fail).

### Creation Commands

| Function | Signature | Description |
|---|---|---|
| `add_app` | `fn(&mut Config, title: String, executable_path: PathBuf) -> ItemId` | Creates an `AppItem` and pushes it to `config.items` (root level). Logs the addition. |
| `add_group` | `fn(&mut Config, title: String) -> ItemId` | Creates a `GroupItem` and pushes it to root. |
| `add_folder` | `fn(&mut Config, title: String, folder_path: PathBuf) -> ItemId` | Creates a `FolderItem` and pushes it to root. |
| `add_app_to_group` | `fn(&mut Config, group_id: ItemId, title: String, executable_path: PathBuf) -> Result<ItemId, String>` | Finds the group by ID, creates an `AppItem` inside it. Errors if group not found. |
| `add_folder_to_group` | `fn(&mut Config, group_id: ItemId, title: String, folder_path: PathBuf) -> Result<ItemId, String>` | Same as above but for folders. |

### Mutation Commands

| Function | Signature | Description |
|---|---|---|
| `remove_item` | `fn(&mut Config, id: ItemId) -> Result<(), String>` | Removes the item by ID, searching root and all groups. Logs the removal. Errors if not found. |
| `rename_item` | `fn(&mut Config, id: ItemId, new_title: String) -> Result<(), String>` | Renames any item type (App, Group, Folder) at root or in any group. |
| `set_icon` | `fn(&mut Config, id: ItemId, icon_path: PathBuf) -> Result<(), String>` | Sets a custom icon path on any item type, at root or in any group. |
| `move_to_group` | `fn(&mut Config, item_id: ItemId, group_id: ItemId) -> Result<(), String>` | Moves an app from anywhere into a target group. Rejects moving groups or folders. |
| `delete_group` | `fn(&mut Config, group_id: ItemId) -> Result<(), String>` | Deletes a group and all its contents from root. Errors if the ID points to a non-group item. |

### Internal Helpers (private)

| Function | Description |
|---|---|
| `rename_if_match(item, id, new_title) -> Option<String>` | Pattern matches the variant and ID, returns the old title on success. |
| `set_icon_if_match(item, id, path) -> bool` | Sets `icon_path` if ID matches. Returns `true` on success. |
| `find_group_mut(config, group_id) -> Option<&mut GroupItem>` | Searches root items for a matching group. |
| `remove_from_anywhere(config, id) -> Option<LaunchItem>` | Finds and removes an item from root or any group. Used by `move_to_group`. |

### Tests (7 total)

All in `#[cfg(test)] mod tests`:

| Test | What it verifies |
|---|---|
| `test_add_and_remove_app` | Add app → 1 item → remove → 0 items. |
| `test_add_and_delete_group` | Add group → 1 item → delete → 0 items. |
| `test_move_app_to_group` | App moves from root into group; root has only the group; group contains the app. |
| `test_rename_item` | Rename app; title updates. |
| `test_cannot_move_group_into_group` | `move_to_group(g1, g2)` returns `Err`. |
| `test_remove_nonexistent` | Removing a fake `ItemId` returns `Err`. |
| `test_config_json_roundtrip` | Serialize → deserialize preserves items. |

---

## Module: `platform/`

### `platform/tray.rs`

#### `TrayEvent`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    Toggle,
    Quit,
}
```

Sent from the tray menu listener thread to the app via crossbeam channel.

#### `create_tray`

```rust
pub fn create_tray(icon: Icon, tx: Sender<TrayEvent>, hotkey: &str) -> Result<TrayIcon, Box<dyn Error>>
```

1. Creates two menu items: "Show / Hide" and "Quit".
2. Builds a `TrayIcon` with the provided icon and tooltip showing the current hotkey.
3. Spawns a listener thread that blocks on `MenuEvent::receiver().recv()`. On Toggle, sends `TrayEvent::Toggle` through the channel and calls `wake_ui()` to nudge egui. On Quit, calls `std::process::exit(0)` immediately.

Uses `tray_icon` crate (v0.24).

### `platform/hotkey.rs`

#### `parse_hotkey`

```rust
pub fn parse_hotkey(s: &str) -> Option<HotKey>
```

Parses a human-readable hotkey string like `"Ctrl+Alt+R"`. Supports `Ctrl`/`Control`, `Alt`, `Shift`, `Win`/`Super`/`Windows` modifiers plus A–Z, 0–9, F1–F12, Space, Tab, Esc, Enter, Backspace, Delete, arrows, Home, End, PageUp/Down, numpad, and function keys.

#### `register_hotkey`

```rust
pub fn register_hotkey(tx: Sender<()>, hotkey_str: &str) -> Result<GlobalHotKeyManager, Box<dyn Error>>
```

1. Creates a `GlobalHotKeyManager`.
2. Parses the hotkey string and registers it.
3. Spawns a listener thread that blocks on `GlobalHotKeyEvent::receiver().recv()`. Filters for `HotKeyState::Released` only (to avoid key-down/key-up double-fire). On event, sends `()` through the channel and calls `crate::app::wake_ui()` to nudge egui.
4. Returns the manager. The caller uses `mem::forget` to keep it alive.

Uses `global_hotkey` crate (v0.8).

### `platform/icons.rs`

#### `IconExtractor` trait

```rust
pub trait IconExtractor {
    fn extract_icon(&self, path: &Path, size: u32) -> Option<Vec<u8>>;
}
```

Platform abstraction for extracting icons from executable files. Returns RGBA8 pixel data.

#### `DummyExtractor`

Non-Windows stub. `extract_icon` always returns `None`.

#### `WindowsIconExtractor` (Windows only)

`#[cfg(windows)]`-gated. Currently also returns `None` — full `HICON`-to-RGBA extraction is deferred.

### `platform/autostart.rs`

#### `set_auto_start`

```rust
pub fn set_auto_start(enable: bool) -> bool
```

Writes or removes a `REG_SZ` value under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` named `Launchpad` pointing to the current executable path. Returns `true` on success.

#### `is_auto_start_enabled`

```rust
pub fn is_auto_start_enabled() -> bool
```

Checks whether the registry value exists using `RegOpenKeyExW` + `RegQueryValueExW`. Used on startup to sync `config.auto_start` with reality.

---

## Module: `ui/`

### `ui/theme.rs`

#### `apply_theme`

```rust
pub fn apply_theme(ctx: &egui::Context, config: &Config)
```

Applies the visual theme from config:

1. Calls `config.resolve_theme()` to get the effective theme.
2. Sets `window_fill` and `panel_fill` to the theme's `body_color`.
3. Applies `corner_radius` from the theme to window and menu corners.
4. Uses `widget_color` for widget backgrounds, `selection_color` for selection highlights.
5. Applies window shadow, disables striped rows and indent lines.
6. All colors fall back to built-in defaults if the theme doesn't specify them.

### `ui/icons.rs`

#### `IconKey`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconKey {
    Custom(PathBuf, u32),        // custom icon path + size
    DefaultApp(PathBuf, u32),    // executable path + size — for extraction
    DefaultGroup,                // yellow folder icon
    DefaultFolder,               // blue folder icon
}
```

Used as the cache key in `IconCache`. The size is part of the key so different icon sizes don't collide. Groups and folders now have distinct default icons (yellow vs blue).

#### `IconCache`

```rust
pub struct IconCache {
    textures: HashMap<IconKey, TextureHandle>,
}
```

| Method | Signature | Description |
|---|---|---|
| `new` | `fn() -> Self` | Creates an empty cache. |
| `get_or_load` | `fn(&mut self, key: IconKey, ctx: &Context) -> Option<&TextureHandle>` | Returns cached texture or generates + caches it. For `DefaultApp`, tries to extract the icon from the exe first, falls back to the generated app icon. |
| `key_for` | `fn(item: &LaunchItem, icon_size: u32) -> IconKey` | Maps a `LaunchItem` to the appropriate key. Apps → `DefaultApp`, Groups → `DefaultGroup`, Folders → `DefaultFolder`. |

#### Icon extraction (Windows)

`extract_icon_rgba(path, size) -> Option<Vec<u8>>` uses `CreateDIBSection` (not `CreateCompatibleBitmap`) to get a real 32bpp buffer with a proper alpha channel. The DIB section's pixel buffer is read directly — no `GetDIBits` conversion needed. BGRA is swapped to RGBA after reading.

#### Private Icon Generators

| Function | Description |
|---|---|
| `generate_default_app_icon(ctx) -> TextureHandle` | 64×64 rounded rectangle with a gray accent border inset. |
| `generate_default_group_icon(ctx) -> TextureHandle` | 64×64 rounded folder shape in yellow (`#F2C94C`). |
| `generate_default_folder_icon(ctx) -> TextureHandle` | 64×64 rounded folder shape in blue (`#64B4FF`). |
| `load_icon_from_file(path, size, ctx) -> Option<TextureHandle>` | Loads a PNG/image from disk, resizes with Lanczos3, and uploads as a texture. |

### `ui/grid.rs`

#### `GridConfig`

```rust
#[derive(Clone)]
pub struct GridConfig {
    pub item_size: Vec2,
    pub spacing: f32,
}
```

Default: `item_size = (96, 110)`, `spacing = 12.0`.

#### `GridOutput`

```rust
pub struct GridOutput {
    pub clicks: Vec<usize>,
    pub double_clicks: Vec<usize>,
}
```

Collects indices of clicked and double-clicked items.

#### `show_grid`

```rust
pub fn show_grid(
    ui: &mut Ui,
    items: &[LaunchItem],
    selected_index: Option<usize>,
    config: &GridConfig,
    render_item: &mut dyn FnMut(&mut Ui, &LaunchItem, bool, Vec2) -> Response,
) -> GridOutput
```

Renders a responsive, centered grid:

1. Computes column count from available width.
2. Calculates horizontal offset to center the grid.
3. Wraps everything in a vertical `ScrollArea`.
4. Iterates items row by row, allocating child UIs per cell.
5. Calls the `render_item` closure for each cell, collecting click/double-click indices.

#### `columns(available_width, item_width, spacing) -> usize`

Private helper. Computes how many columns fit, with a minimum of 1.

### `ui/item_card.rs`

#### `render_item_card`

```rust
pub fn render_item_card(
    ui: &mut Ui,
    item: &LaunchItem,
    is_selected: bool,
    desired_size: Vec2,
    icon_cache: &mut IconCache,
) -> Response
```

Renders a single card:

- Allocates exact space with `Sense::click()`.
- Background: transparent / hover (dark highlight) / selected (brighter highlight).
- Selected border: 2px blue stroke.
- Icon: loaded from `IconCache` at 48px, drawn centered above the label.
- Label: truncated to 14 chars + ellipsis, centered below the icon.
- Returns the `Response` for the caller to check clicks.

### `ui/launcher.rs`

#### `LauncherUI`

```rust
pub struct LauncherUI;
```

Stateless namespace struct.

| Method | Signature | Description |
|---|---|---|
| `show` | `fn(ui, config, nav_stack, selected_index, icon_cache) -> LauncherResponse` | Renders the full launcher: navigation bar → separator → grid (or empty state). |
| `render_nav_bar` (private) | `fn(ui, config, nav_stack)` | Paints a breadcrumb bar: "Launchpad" (blue) → `▸` → group names. Uses painter galley calls for precise layout. |

#### `LauncherResponse`

```rust
#[derive(Default)]
pub struct LauncherResponse {
    pub clicked_index: Option<usize>,
    pub clicked_id: Option<ItemId>,
    pub double_clicked_id: Option<ItemId>,
}
```

Aggregates interaction results from grid rendering.

#### `find_group_title` (private)

```rust
fn find_group_title(config: &Config, id: ItemId) -> Option<String>
```

Looks up a group's title by ID for breadcrumb display.

### `ui/context_menu.rs`

```rust
pub struct ContextMenu;
```

A placeholder module for Phase 5 context menu logic. Currently empty — context menus are rendered inline in `app.rs::render_context_menu`.

---

## Module: `utils.rs`

**Path:** `src/utils.rs`

### `generate_tray_icon`

```rust
pub fn generate_tray_icon() -> tray_icon::Icon
```

Programmatically generates a 32×32 tray icon:

1. Creates an `RgbaImage` with rounded-corner background (`#1E1E2E`).
2. Draws a stylized "L" shape in accent blue (`#89B4FA`): vertical stroke at x=8 (y 8–24), horizontal stroke at y=24 (x 8–24).
3. Converts to raw RGBA bytes and constructs a `tray_icon::Icon::from_rgba`.

This avoids needing an external PNG file bundled with the binary.

---

## Recent Features (app.rs)

### Search

A search text box on the right of the breadcrumb bar. `search_all_items()` recursively scans root items and all groups for case-insensitive title matches. When searching, the nav stack is cleared to show a flat grid of results.

- `Escape` clears the search query.
- `Enter` activates the selected result.
- Changed via `search_query` field on `LaunchpadApp`.

### Auto-Fit Icons

Toggle button in the title bar (outward arrows icon). When active, `compute_fit_icon_size()` runs every frame using `ui.available_width()` and `ui.available_height()` via binary search to find the largest icon size where all items fit without scrolling. Disabled by changing icon size, spacing, or theme.

### Icon Migration

On first frame, `find_external_icons()` scans all items for `icon_path` values pointing outside the `icons/` directory (old-format full paths). If any are found, a dialog offers one-click migration: each file is copied into `icons/` with a UUID name, and the path is updated to just the filename.

### Relative Icon Paths

Icon paths are stored as bare filenames (e.g. `"uuid.png"`). `IconCache::resolve_path()` prepends the `icons/` directory at load time. Full paths still work for backward compatibility.

### Movable Dialogs

All modal windows (Settings, Reorder, Rename, Confirm Delete, Select Group) use `.movable(true).constrain(false).default_pos(center)` — they start centered and can be dragged anywhere, including outside the main window bounds.

### Window Border

An overlay `Area` draws a 1px rounded rect around the viewport edge, giving the frameless main window a visible outline matching the theme's divider color.

---

## Dependencies (`Cargo.toml`)

| Crate | Version | Purpose |
|---|---|---|
| `eframe` / `egui` | 0.31 | GUI framework and windowing. |
| `tray-icon` | 0.24 | System tray icon and menu. |
| `global-hotkey` | 0.8 | Global keyboard shortcut registration. |
| `crossbeam` | 0.8 | Multi-producer channels for event passing. |
| `serde` / `serde_json` | 1 | Serialization for config persistence. |
| `dirs` | 6 | Platform config directory resolution. |
| `image` | 0.25 | Icon loading from disk (PNG support). |
| `uuid` | 1 (v4 + serde) | Stable unique IDs for items. |
| `log` / `env_logger` | 0.4 / 0.11 | Structured logging. |
| `windows` | 0.58 | Windows-specific shell/COM APIs (icon extraction — deferred). |

### Release Profile

- `opt-level = 2`
- `lto = true`
- `codegen-units = 1`
- `strip = true`

Optimized for small binary size.
