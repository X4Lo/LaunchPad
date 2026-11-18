# Launchpad — Technical Reference

How the pieces fit together. For the file layout see [folder-structure.md](folder-structure.md).

## Stack

| Concern | Choice | Why |
|---|---|---|
| UI | [Slint](https://slint.dev) 1.18, `winit` backend, **software renderer**, Fluent style | Retained mode (only dirty regions repaint), no GPU context, declarative `.slint` markup compiled to Rust at build time. Same UI system as HushKey. |
| Tray | `tray-icon` 0.26 | Created on the main thread once the event loop runs. |
| Hotkey | `global-hotkey` 0.10 (`RegisterHotKey`) | Re-registered live when changed — no restart. |
| Autostart | `auto-launch` (HKCU `Run` value `Launchpad`, with `--minimized`) | No admin rights. |
| Icons | Windows shell (`IShellItemImageFactory`, `SHDefExtractIconW`) + `image` for custom files | See *Icons* below. |
| Dialogs | `rfd`, run on a helper thread | The UI keeps painting while the picker is open. |

## Threads

- **Main thread** — Slint event loop, `App` controller, tray, hotkey manager.
- **Icon workers** (2–4) — COM STA each; turn icon requests into RGBA and post them back with `slint::invoke_from_event_loop`.
- **Open helpers** — one short-lived thread per launch (`ShellExecuteW "open"`), so a slow target never freezes the window.
- **Single-instance listener** — a loopback socket (port 47221); a second launch just shows the running window.

## Controller (`src/app.rs`)

`App` lives in a thread-local `Rc`; every Slint callback reaches it with `with_app(|a| …)`, and other threads use `on_ui(|a| …)`.

- The launcher window is created **once** and shown/hidden, so the hotkey is instant. The Settings window is built on demand and destroyed when closed.
- `refresh()` rebuilds the `Tile` model for the current group or search. When the tile count is unchanged it updates rows in place, so an icon arriving mid-drag doesn't recreate the tile under the pointer.
- Config edits go through `edit(|cfg| …)`: mutate → debounced save (600 ms) → refresh.
- Window geometry is captured from winit `Moved`/`Resized` events (via `on_winit_window_event`) and on hide/quit, stored in logical pixels.
- Files dropped from Explorer arrive as winit `DroppedFile` events (one per file); they're batched and added to the current group.
- Mouse Back / Forward buttons (winit `MouseInput`) leave a group / re-enter the one just left.

## UI (`ui/*.slint`)

- `theme.slint` — the `Theme` global (design tokens). Its `ThemeColors` struct is pushed from Rust (`src/ui/theme.rs`) into **each** window, because Slint globals are per window.
- `widgets.slint` — shared building blocks: stroke glyphs (each a constant SVG path, validated at compile time), `IconButton`, `Button`, `TextField`, `Tabs`, `Switch`, `ShortcutChip`, `SettingRow`, `Card`, `Slider`, `MenuEntry`, `MenuPanel`.
- `main.slint` — `MainWindow`: header, search, the icon grid (positions computed from the window width; auto-fit asks Rust via a `pure callback`), drag-to-reorder / drop-into-group, item & add menus (`PopupWindow`), modal dialogs, drop overlay, status banner with Undo.
- `settings.slint` — `SettingsWindow` with General / Appearance / About tabs and shortcut recording.

Keyboard handling uses `FocusScope.capture-key-pressed`, so arrows / Enter / Esc drive the grid while typed characters still reach the search box.

## Icons (`src/platform/shell.rs`)

1. `IShellItemImageFactory::GetImage(128 px, ICONONLY)` — what Explorer uses; resolves `.lnk` targets and folders. `E_PENDING` (icon cache still warming) is retried.
2. If that fails or the bitmap has no alpha (legacy 24-bit icons), the icon handle is read directly (`SHDefExtractIconW`, falling back to `SHGetFileInfoW`) and transparency comes from its AND mask.
3. Small icons that the shell centers on a big empty canvas are cropped to fill the tile.
4. Custom icons: PNG/ICO/JPG/BMP via `image`; anything else (e.g. an `.exe` chosen as icon) goes through the shell path.

Results are cached per source for the session. Items without an icon show a coloured letter tile; items whose target no longer exists get a red **!** badge. Groups show up to four member icons.

`$(Drive)` in stored paths expands to the drive Launchpad runs from (portable-launcher convention).

## Item tree (`src/commands/items.rs`)

Root holds apps, folders and groups; groups hold apps and folders (no nesting). All mutations (`add_*`, `move_to_group`, `move_to_root`, `ungroup`, `reorder`, `rename_item`, `set_icon`, `remove_item`, `search`) keep item ids and custom icons intact and are unit-tested.

## Config (`src/config/manager.rs`)

JSON, unchanged format (older configs load as-is). Portable mode = `config.json` next to the exe; otherwise `%APPDATA%\Launchpad\config.json`. Saves are atomic (write `.tmp`, rename). Copies of the old built-in themes that the egui version wrote into `themes` are dropped on load.

## Commands

```bash
cargo build                               # debug build
cargo run                                 # run; `-- --minimized` starts in the tray
cargo test                                # unit tests
cargo clippy --all-targets -- -D warnings # lint
cargo fmt                                 # rustfmt.toml, width 120
```
