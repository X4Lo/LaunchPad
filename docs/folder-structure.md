# Launchpad — Folder Structure

```
Launchpad/
├── Cargo.toml                  # Rust project manifest + dependencies
├── Cargo.lock                  # Dependency lock file
├── README.md                   # Project overview and usage guide
├── .gitignore
├── Prompt.md                   # Original project prompt / spec
│
├── docs/                       # Documentation
│   ├── Architecture & Implementation Plan.md
│   ├── phase-1-foundation.md
│   ├── phase-2-config-data-model.md
│   ├── phase-3-grid-ui.md
│   ├── changelog-recent.md
│   ├── technical-reference.md  # Detailed module reference
│   └── folder-structure.md     # This file
│
├── resources/                  # Bundled assets (icons, images)
│
└── src/                        # Application source code
    ├── main.rs                 # Entry point: init logging, config, tray, hotkey, egui
    │
    ├── app.rs                  # LaunchpadApp: state, eframe::App impl, all rendering
    │
    ├── config/
    │   ├── mod.rs
    │   └── manager.rs          # Config, Theme, ConfigManager (load/save/portable)
    │
    ├── models/
    │   ├── mod.rs              # Re-exports
    │   ├── item.rs             # ItemId, LaunchItem enum
    │   ├── app_item.rs         # AppItem struct
    │   ├── group_item.rs       # GroupItem struct
    │   └── folder_item.rs      # FolderItem struct
    │
    ├── commands/
    │   ├── mod.rs
    │   └── items.rs            # CRUD operations on config items + tests
    │
    ├── platform/
    │   ├── mod.rs
    │   ├── tray.rs             # System tray icon + menu
    │   ├── hotkey.rs           # Global hotkey (Ctrl+Alt+R)
    │   └── icons.rs            # IconExtractor trait (platform abstraction)
    │
    ├── ui/
    │   ├── mod.rs
    │   ├── theme.rs            # Visual theme from Config
    │   ├── icons.rs            # IconCache, IconKey, extraction + generation
    │   ├── grid.rs             # Grid layout helper
    │   ├── item_card.rs        # Single item card rendering
    │   ├── launcher.rs         # Full launcher layout
    │   └── context_menu.rs     # Placeholder (menus are inline in app.rs)
    │
    └── utils.rs                # generate_tray_icon()
```

## Key design decisions

- **Everything in `app.rs`** — The rendering, event handling, and state management live in `LaunchpadApp` to keep egui's immediate-mode flow simple. Sub-modules handle data models and pure logic.
- **`thread_local!` CTX** — The egui `Context` is stored in a thread-local so non-`update` methods can send viewport commands (e.g., minimize on launch).
- **Atomic config saves** — Writes go to a `.tmp` file, then rename over the real path to prevent corruption.
- **Portable mode** — If `config.json` exists next to the `.exe`, the app uses it instead of `%APPDATA%`.
