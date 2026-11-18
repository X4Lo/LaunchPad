# Launchpad — Folder Structure

```
Launchpad/
├── Cargo.toml                  # Manifest (Slint, tray-icon, global-hotkey, windows, …)
├── build.rs                    # Compiles ui/main.slint; embeds the exe icon (.ico/.res)
├── rustfmt.toml                # width 120 (same as HushKey)
├── README.md
├── docs/
│   ├── technical-reference.md  # How it works (threads, controller, UI, icons)
│   ├── themes.md               # Built-in and custom themes
│   ├── changelog-recent.md
│   ├── folder-structure.md     # This file
│   └── Architecture & Implementation Plan.md   # Original (egui-era) plan, historical
├── resources/                  # Screenshot for the README
├── ui/                         # Slint markup, compiled at build time
│   ├── theme.slint             # Theme global + ThemeColors tokens
│   ├── widgets.slint           # Glyphs, buttons, fields, switches, sliders, menus
│   ├── main.slint              # MainWindow: header, search, grid, menus, dialogs
│   └── settings.slint          # SettingsWindow: General / Appearance / About
└── src/
    ├── main.rs                 # Logging, single instance, backend selection, event loop
    ├── app.rs                  # App controller: state, callbacks, windows, dialogs
    ├── models/                 # AppItem, FolderItem, GroupItem, LaunchItem, ItemId
    ├── commands/items.rs       # Item-tree mutations + search (unit-tested)
    ├── config/manager.rs       # Config, custom Theme, ConfigManager, $(Drive) expansion
    ├── platform/
    │   ├── shell.rs            # Launching, Show in Explorer, icon extraction workers
    │   ├── hotkey/             # Global hotkey + Accelerator parsing/capture
    │   ├── tray.rs             # Tray icon and menu
    │   ├── autostart.rs        # Start with Windows (HKCU Run)
    │   └── instance.rs         # Single-instance guard
    └── ui/
        ├── icon.rs             # Procedural app icon (std-only; shared with build.rs)
        ├── theme.rs            # Built-in palettes, custom-theme mapping
        └── mod.rs              # Image helpers, native window chrome, auto-fit sizing
```

`ui/*.slint` is markup; `src/ui/` is the Rust side of the UI. Platform code stays in `src/platform/`, so `app.rs` never calls Win32 directly.
