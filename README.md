# Launchpad

A lightweight, minimalistic desktop application launcher for Windows. Keep your most-used apps one hotkey away — no more hunting through the Start menu or cluttered desktops.

> 🤖 This project was entirely AI-coded (Zed Agent + DeepSeek V4 Pro + Claude) — from architecture design through implementation, UI polish, and documentation.

## Features

- **Global hotkey** — Configurable shortcut (default `Ctrl+Alt+R`) toggles window visibility
- **System tray** — Runs quietly in the background with Show/Hide and Quit menu
- **Themes** — Built-in Default, Dracula, Nord, and Catppuccin themes with full color/spacing customization
- **Search** — Search box filters all items across groups in real time
- **Drag & drop** — Drop executables or shortcuts to add them instantly
- **Groups & folders** — Organize apps into navigable groups and filesystem folder shortcuts
- **Custom icons** — Assign custom icons, auto-extracted from executables, stored locally as portable filenames
- **Reorder** — List view with up/down buttons to reorder items at any level
- **Auto-fit** — Toggle that dynamically resizes icons to fill the window
- **Context menus** — Right-click to rename, change icon, move to group, or remove
- **Resizable** — Drag bottom/right/corner edges or use the fit button
- **Auto-start with Windows** — Registry-based, toggle in Settings
- **Portable mode** — `config.json` next to the exe takes priority over `%APPDATA%`

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
      "items": [
        {
          "type": "app",
          "id": "...",
          "title": "Google Chrome",
          "executable_path": "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
          "icon_path": null
        }
      ]
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
  "hide_on_launch": false,
  "hotkey": "Ctrl+Alt+R"
}
```

Window position and size are saved automatically whenever you move or resize the launcher.

### Hotkey

The global hotkey toggles Launchpad visibility (show/hide). It defaults to `Ctrl+Alt+R`.

You can change it in `config.json`:

```json
{
  "hotkey": "Ctrl+Shift+F"
}
```

Or edit it from **Settings > General > Global Hotkey**. The change takes effect after restart.

**Supported modifiers:** `Ctrl`, `Alt`, `Shift`, `Win`/`Super`

**Supported keys:** `A`–`Z`, `0`–`9`, `F1`–`F12`, `Space`, `Tab`, `Esc`, `Enter`, `Backspace`, `Delete`, arrow keys (`Up`/`Down`/`Left`/`Right`), `Home`, `End`, `PageUp`/`PageDown`, `PrintScreen`, `ScrollLock`, `Pause`, `Insert`, `CapsLock`, `NumLock`, numpad keys (`Num0`–`Num9`, `NumAdd`, `NumSubtract`, `NumMultiply`, `NumDivide`, `NumDecimal`, `NumEnter`).

**Examples:**

- `Ctrl+Alt+R` (default)
- `Ctrl+Shift+F`
- `Win+Space`
- `Alt+F12`

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

| Document                                                   | Description                                            |
| ---------------------------------------------------------- | ------------------------------------------------------ |
| [docs/themes.md](docs/themes.md)                           | Theme system: fields, built-in themes, custom creation |
| [docs/folder-structure.md](docs/folder-structure.md)       | Project directory layout                               |
| [docs/technical-reference.md](docs/technical-reference.md) | Module-by-module API reference                         |
| [docs/changelog-recent.md](docs/changelog-recent.md)       | Recent feature additions and fixes                     |

## Keyboard shortcuts

| Key              | Action                               |
| ---------------- | ------------------------------------ |
| `Ctrl + Alt + R` | Toggle Launchpad visibility          |
| `Esc`            | Close context menu / go back to root |
| `Backspace`      | Navigate up one level                |
| `Enter`          | Open selected item                   |
| Arrow keys       | Navigate grid items                  |

## Auto-start with Windows

Enable **Start with Windows** in Settings > General. Launchpad writes a registry value at:

```
HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run
    "Launchpad" = "C:\path\to\launchpad.exe"
```

When disabled, the value is removed. No admin rights required.

## Item types

| Type       | Icon                                  | Behavior                           |
| ---------- | ------------------------------------- | ---------------------------------- |
| **App**    | Extracted from executable (or custom) | Launches the application           |
| **Group**  | Yellow folder (or custom)             | Navigates into a sub-grid of items |
| **Folder** | Blue folder (or custom)               | Opens the folder in Explorer       |

## Building

**Requirements:** Rust 1.75+

```bash
cargo build # or
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
