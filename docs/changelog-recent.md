# Changelog — Recent Major Update

## New Features

### Theme System
- New `Theme` struct with fields: `name`, `header_color`, `body_color`, `widget_color`, `selection_color`, `divider_color`, `text_color`, `corner_radius`, `grid_spacing`, `grid_icon_size`.
- `Config` now has `themes: Vec<Theme>` and `selected_theme: Option<String>`.
- Four built-in themes: **Default**, **Dracula**, **Nord**, **Catppuccin**.
- `Config::resolve_theme()` merges selected theme over Default; falls back gracefully if theme name not found.
- `ConfigManager::load()` auto-merges built-in themes into user config.
- Settings now has a **Themes** tab with theme list and color swatch previews.
- Theme-aware rendering: title bar, body, divider, grid spacing/icon size all respect the selected theme.
- Window corner radius configurable per theme.

### Window Position & Size — Immediate Save
- Window position and size are now saved **on every change**, not just on app close.
- `update()` compares current viewport rect against stored values and marks dirty only when they differ.

### Delete Confirmation for Non-Empty Groups
- Deleting a group that contains items now shows a confirmation dialog with the group name and item count.
- Empty groups still delete instantly (no confirmation needed).
- New `pending_delete_group` field in `LaunchpadApp`.

### Resize Handles — Always Reachable
- Resize handles moved from inside `CentralPanel` to separate overlay `Area`s at window edges.
- Each edge (right, bottom, corner) is a 6px strip that stays reachable regardless of window size.
- Safety auto-reset: if resize state gets stuck (mouse released outside window), it clears on the next frame.

### Portable Mode
- If `config.json` exists next to the executable, `ConfigManager::new()` uses it instead of `%APPDATA%`.
- No directories created in `%APPDATA%` when portable mode is active.

### Icon Extraction Fix
- Switched from `CreateCompatibleBitmap` to `CreateDIBSection` for proper 32bpp alpha channel.
- Pixels read directly from the DIB section buffer (no `GetDIBits` conversion).
- Buffer zeroed before drawing so uncovered pixels are transparent.

### UI Polish
- Distinct folder colors: groups are yellow (`#F2C94C`), folders are blue (`#64B4FF`).
- Title bar spans full width with theme-colored panel frame.
- Breadcrumb shows folder/group emoji icons (📂 / 📁) next to names.
- Removed back arrow from title bar; removed 🏠 emoji from breadcrumb home link.
- Grid hover/selection uses neutral gray instead of blue.
- Custom divider line replaces default `ui.separator()`.
- Color scheme updated: header `#313337`, body `#1F2127`.

## Fixes

### Close Button
- The close button now draws a literal **X** using two `painter.line_segment()` calls instead of relying on a Unicode character (`×`) that may not render consistently across fonts and platforms.
- Hover state turns the X bright red for visual feedback.

### Context Menu Position
- The context menu now uses `.fixed_pos(state.pos)` with the cursor position captured at right-click time, rather than following the cursor continuously.

### Context Menu Width
- Width is clamped to 150 px via `.default_width(150.0)` and `ui.set_min_width(140.0)`, preventing overly wide menus.

### Tray Icon Event Handling
- Switched from per-item polling to `MenuEvent::set_event_handler` for reliable, callback-based tray menu event delivery.

## Data Model Changes

| Change | Detail |
|---|---|
| `Theme` struct | New model with name, colors, corner_radius, grid_spacing, grid_icon_size |
| `Config.themes` | `Vec<Theme>` — available themes (defaults to built-in set) |
| `Config.selected_theme` | `Option<String>` — name of the active theme |
| `FolderItem` struct | New model: `id`, `title`, `icon_path`, `folder_path` |
| `LaunchItem::Folder` variant | Added to the `LaunchItem` enum with `#[serde(rename = "folder")]` |
| `IconKey::DefaultFolder` | New variant for blue folder icon |
| `Config.window_x` | `Option<f32>` — saved outer-position x |
| `Config.window_y` | `Option<f32>` — saved outer-position y |
| `Config.grid_spacing` | `f32` — configured grid cell spacing (default 12.0) |
| `Config.grid_icon_size` | `f32` — configured icon size in px (default 48.0) |
| `Config.hide_on_launch` | `bool` — minimize-on-launch toggle (default false) |

All new fields use `#[serde(default)]` for backward compatibility with existing `config.json` files.

## Build

- **0 warnings** across the full workspace.
- **7 tests passing** in `commands::items::tests`:
  - `test_add_and_remove_app`
  - `test_add_and_delete_group`
  - `test_move_app_to_group`
  - `test_rename_item`
  - `test_cannot_move_group_into_group`
  - `test_remove_nonexistent`
  - `test_config_json_roundtrip`
