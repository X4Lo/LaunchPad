# Launchpad

A lightweight, minimalistic desktop application launcher for Windows. Keep your most-used apps one hotkey away — no more hunting through the Start menu or cluttered desktops.

## Features

- **Global hotkey** — Press `Ctrl + Alt + R` to summon Launchpad instantly, from anywhere
- **Always on top** — The launcher floats above all other windows
- **System tray** — Runs quietly in the background; right-click the tray icon to toggle or quit
- **Drag & drop** — Drop executables or shortcuts directly onto Launchpad to add them
- **Groups & folders** — Organize apps into groups (navigable sub-grids) and folders (filesystem shortcuts)
- **Custom icons** — Assign custom icons to any item; executable icons are extracted automatically
- **Context menus** — Right-click any item to rename, change icon, move to group, or remove
- **Resizable window** — Drag the bottom, right, or corner edges to resize
- **Settings** — Adjust grid spacing, icon size, and auto-hide-on-launch behavior
- **Portable mode** — Drop a `config.json` next to the executable and Launchpad uses it instead of the system config directory

## Installation

### From source

```bash
git clone <repo-url>
cd Launchpad
cargo build --release
```

The binary will be at `target/release/launchpad.exe`.

### Portable mode

Place a `config.json` file in the same directory as `launchpad.exe`. Launchpad will automatically detect and use it, keeping all configuration self-contained alongside the executable — no files written to `%APPDATA%`.

## Configuration

Configuration is stored as JSON. The default location is `%APPDATA%\Launchpad\config.json`.

### Config structure

```json
{
  "items": [
    {
      "type": "app",
      "id": "...",
      "title": "Google Chrome",
      "executable_path": "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
      "icon_path": null
    },
    {
      "type": "group",
      "id": "...",
      "title": "Games",
      "icon_path": null,
      "items": []
    },
    {
      "type": "folder",
      "id": "...",
      "title": "Documents",
      "folder_path": "C:\\Users\\Me\\Documents",
      "icon_path": null
    }
  ],
  "window_x": 100,
  "window_y": 200,
  "window_width": 640,
  "window_height": 480,
  "grid_spacing": 12,
  "grid_icon_size": 48,
  "hide_on_launch": false
}
```

Window position and size are saved automatically whenever you move or resize the launcher.

### Themes

Launchpad supports themes for customizing colors, corner radius, and grid layout. See [docs/themes.md](docs/themes.md) for full documentation.

Quick example — add this to your `config.json`:

```json
{
  "themes": [
    {
      "name": "My Custom Theme",
      "header_color": "2E3440",
      "body_color": "242933",
      "widget_color": "3B4252",
      "selection_color": "5E81AC",
      "corner_radius": 10
    }
  ],
  "selected_theme": "My Custom Theme"
}
```

Built-in themes: **Default**, **Dracula**, **Nord**, **Catppuccin**.

## Project structure

See [docs/folder-structure.md](docs/folder-structure.md) for the full directory layout and design rationale.

## Documentation

| Document | Description |
|----------|-------------|
| [docs/themes.md](docs/themes.md) | Theme system: fields, built-in themes, custom creation |
| [docs/folder-structure.md](docs/folder-structure.md) | Project directory layout |
| [docs/technical-reference.md](docs/technical-reference.md) | Module-by-module API reference |
| [docs/changelog-recent.md](docs/changelog-recent.md) | Recent feature additions and fixes |

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Ctrl + Alt + R` | Toggle Launchpad visibility |
| `Esc` | Close context menu / go back to root |
| `Backspace` | Navigate up one level |
| `Enter` | Open selected item |
| Arrow keys | Navigate grid items |

## Item types

| Type | Icon | Behavior |
|------|------|----------|
| **App** | Extracted from executable (or custom) | Launches the application |
| **Group** | Yellow folder (or custom) | Navigates into a sub-grid of items |
| **Folder** | Blue folder (or custom) | Opens the folder in Explorer |

## Building

**Requirements:** Rust 1.75+

```bash
cargo build --release
```

The release binary is optimized with LTO and stripped for minimal size.

## Tech stack

- **[egui](https://github.com/emilk/egui) / eframe** — Immediate-mode GUI, frameless window
- **[tray-icon](https://crates.io/crates/tray-icon)** — System tray integration
- **[global-hotkey](https://crates.io/crates/global-hotkey)** — Global keyboard shortcut registration
- **[serde](https://serde.rs) / serde_json** — Configuration serialization
- **[image](https://crates.io/crates/image)** — Custom icon loading (PNG)
- **[windows](https://crates.io/crates/windows)** — Win32 API for icon extraction
- **[rfd](https://crates.io/crates/rfd)** — Native file dialogs
- **[crossbeam](https://crates.io/crates/crossbeam)** — Channel-based event passing

## License

MIT
