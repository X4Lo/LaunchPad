I'll start by examining the current state of the project directory, then produce a comprehensive architecture and planning document.

The project is empty except for the prompt. Let me now produce a thorough architecture and planning document. I'll research the crate ecosystem quickly to validate my recommendations.

Both `tray-icon` (v0.24.1, ~19M downloads) and `global-hotkey` (v0.8.0, ~3.6M downloads) are active and well-maintained by the Tauri project. Now I have everything I need to produce the full architecture document.

---

# Launchpad — Architecture & Implementation Plan

## 1. High-Level Architecture

```
┌──────────────────────────────────────────────────────┐
│                      Launchpad                       │
├───────────────┬──────────────────────────────────────┤
│  Process      │  System Tray Icon                    │
│  Lifetime     │  (tray-icon crate)                   │
│               │                                      │
│               │  Global Hotkey                       │
│               │  (global-hotkey crate)               │
│               │  Ctrl+Alt+R → toggle window          │
├───────────────┴──────────────────────────────────────┤
│  Main Window (eframe/egui)                           │
│  ┌────────────────────────────────────────────────┐  │
│  │  Navigation Stack                              │  │
│  │  [Root Grid] → [Group: "Games"] → ...          │  │
│  ├────────────────────────────────────────────────┤  │
│  │  Grid View (current level)                     │  │
│  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐          │  │
│  │  │ Icon │ │ Icon │ │ Icon │ │ Icon │          │  │
│  │  │Title │ │Title │ │Title │ │Title │          │  │
│  │  └──────┘ └──────┘ └──────┘ └──────┘          │  │
│  └────────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────────┤
│  Config Manager (serde + JSON)                       │
│  ┌────────────────────────────────────────────────┐  │
│  │  %APPDATA%/LaunchPad/config.json               │  │
│  │  Read on startup / Write on mutation            │  │
│  └────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

**Event flow:**

```
Global Hotkey Press → Channel (Sender) → App State Toggle Visible
      ↓
Main Loop (egui) reads state, renders window if visible
      ↓
User Interaction → mutate Config → auto-save JSON
      ↓
Escape / Focus Lost → hide window (never destroy, just toggle visibility)
```

**Key design decisions:**

- The egui window is created once at startup and toggled visible/hidden. Never destroyed. This makes "appear instantly" trivial — it's just a show + focus call.
- Config is the single source of truth. All mutations write through to `Config`, which auto-serializes to JSON.
- A channel (`std::sync::mpsc` or `crossbeam`) bridges the global hotkey callback (OS thread) to the egui main loop.
- No async runtime needed. Everything is synchronous and runs on the main thread. This keeps the binary small and startup fast.

---

## 2. GUI Framework Recommendation

### **Primary: `eframe` / `egui` v0.30+**

| Criterion | egui | Tauri |
|---|---|---|
| RAM baseline | ~10–20 MB | ~50–100 MB (WebView) |
| Startup time | ~100 ms | ~500–800 ms |
| Binary size | ~5–8 MB | ~15–25 MB |
| Custom look | Full control (immediate mode) | HTML/CSS full control |
| System tray | External crate needed | Built-in |
| Global hotkeys | External crate needed | Built-in |
| Cross-platform | Yes (Linux, macOS, Windows) | Yes |
| Drag-and-drop (files) | Built-in support | Built-in |
| Maturity | Very mature, widely used | Very mature |

**Justification:** The spec calls for "very little RAM," "launch instantly," and "almost zero CPU while idle." egui wins decisively on all three. The custom look (flat, rounded corners) is achievable with egui's styling API and matches the PowerToys Run / Raycast aesthetic better than a native Win32 look would.

**Alternative — Tauri:** If visual polish ever becomes more important than the RAM/startup budget, Tauri is the fallback. Porting the data model and logic would be straightforward since it's the same Rust backend.

### Portability notes

egui compiles and runs on Linux and macOS with zero code changes. The **icon extraction** code is the only platform-specific piece — on Windows we use the `windows` crate to call `ExtractIconExW`, while on Linux/macOS we'd use different approaches. This is cleanly isolated behind a `PlatformIconExtractor` trait.

---

## 3. Crate Recommendations

| Purpose | Crate | Version | Notes |
|---|---|---|---|
| **GUI** | `eframe` / `egui` | 0.30+ | Immediate-mode GUI, glow backend |
| **System tray** | `tray-icon` | 0.24 | Cross-platform, maintained by Tauri team |
| **Global hotkeys** | `global-hotkey` | 0.8 | Cross-platform, maintained by Tauri team |
| **JSON serialization** | `serde` + `serde_json` | 1.x | Standard, derive macros |
| **App data directory** | `dirs` (or `directories`) | 5.x | Get `%APPDATA%` / `~/.config` |
| **Icon extraction (Win)** | `windows` | 0.58+ | Win32 API for `ExtractIconExW`, `SHGetFileInfoW` |
| **`.lnk` parsing (Win)** | `parselnk` | 0.1 | Parse Windows shortcut files from drag-drop |
| **Image loading** | `image` | 0.25 | Load custom icon files (PNG, ICO) |
| **UUID generation** | `uuid` | 1.x | Unique IDs for items (v4) |
| **Crossbeam channel** | `crossbeam` | 0.8 | Or use `std::sync::mpsr` — needed for hotkey → UI thread comm |

**Crates NOT needed:**
- No `tokio` or async runtime
- No database drivers
- No HTTP client
- No filesystem watcher (config is only mutated by us)

---

## 4. Proposed Folder Structure

```
src/
├── main.rs                  # Entry point: init logging, config, tray, hotkey, egui
├── app.rs                   # LaunchpadApp: egui eframe::App impl, top-level state
│
├── config/
│   ├── mod.rs
│   └── manager.rs           # ConfigManager: load/save/auto-save, defaults
│
├── models/
│   ├── mod.rs
│   ├── item.rs              # LaunchItem enum (App | Group), LaunchItemId
│   ├── app_item.rs          # AppItem struct
│   └── group_item.rs        # GroupItem struct
│
├── ui/
│   ├── mod.rs
│   ├── launcher.rs          # Main launcher window: grid + navigation
│   ├── grid.rs              # Grid layout logic, responsive columns
│   ├── item_card.rs         # Individual card rendering (icon + title)
│   ├── context_menu.rs      # Right-click context menu rendering
│   └── theme.rs             # egui visuals / style setup (colors, rounding, shadows)
│
├── platform/
│   ├── mod.rs
│   ├── tray.rs              # System tray setup (icon, menu, show/hide)
│   ├── hotkey.rs            # Global hotkey registration + channel
│   └── icons.rs             # Icon extraction trait + Windows implementation
│
├── commands/
│   ├── mod.rs
│   └── items.rs             # Command functions: add_app, remove_item, rename, move_to_group
│
└── utils.rs                 # Small helpers (title extraction from path, etc.)
```

Compared to the prompt's suggestion, I removed `widgets/`, `icons/`, and `storage/` — `icons` merges into `platform/`, `storage` is just `config/manager.rs`, and egui's immediate mode means "widgets" are just functions in `ui/`.

---

## 5. Data Model

```rust
// models/item.rs

/// Unique identifier for any launch item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub Uuid);

/// Top-level discriminated union
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LaunchItem {
    App(AppItem),
    Group(GroupItem),
}

// models/app_item.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppItem {
    pub id: ItemId,
    pub title: String,
    #[serde(default)]
    pub icon_path: Option<PathBuf>,    // None = use system icon
    pub executable_path: PathBuf,
}

// models/group_item.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupItem {
    pub id: ItemId,
    pub title: String,
    #[serde(default)]
    pub icon_path: Option<PathBuf>,    // None = use default folder icon
    pub items: Vec<LaunchItem>,        // Only App items (no nesting by default)
}
```

**Key points:**
- `serde(tag = "type")` produces clean JSON exactly as specified: `{"type": "app", ...}` or `{"type": "group", ...}`
- `ItemId` wraps a `Uuid` for unique identification — needed for rename/remove operations
- `icon_path: None` means "use default" (system icon for apps, folder icon for groups)
- Groups are flat (only contain `App` items), though the `Vec<LaunchItem>` type allows future nesting if desired

**Config root:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub items: Vec<LaunchItem>,
    #[serde(default)]
    pub window_width: Option<f32>,    // persisted window size
    #[serde(default)]
    pub window_height: Option<f32>,
}
```

---

## 6. State Management Approach

Central state lives in a single struct:

```rust
pub struct LaunchpadApp {
    // Window state
    pub visible: bool,
    pub window_size: egui::Vec2,

    // Data
    pub config: Config,

    // Navigation
    pub nav_stack: Vec<ItemId>,  // stack of group IDs; empty = root
    pub selected_index: Option<usize>,

    // Context menu state
    pub context_menu: Option<ContextMenuState>,

    // Channels
    pub hotkey_rx: Receiver<()>,  // receives from global hotkey thread

    // Icons cache
    pub icon_cache: HashMap<IconKey, egui::TextureHandle>,
}
```

**Rules:**
1. `Config` is the single source of truth for all items
2. Mutations go through `commands/items.rs` functions that take `&mut Config` and return `Result`
3. After any mutation, `ConfigManager::save(&config)` is called
4. The hotkey thread sends `()` on a channel; the main loop checks `hotkey_rx.try_recv()` each frame and toggles visibility
5. Icons are loaded lazily and cached as `egui::TextureHandle` keyed by `(path, size)`

---

## 7. Configuration Management

```
%APPDATA%\LaunchPad\
├── config.json        # Main configuration file
└── icons\             # Cached extracted icons (optional optimization)
```

**Behavior:**
- On startup: read `config.json`. If missing or corrupt, create a default (empty items list).
- On mutation: write atomically (write to `.tmp`, then rename) to prevent corruption.
- No filesystem watching needed — we're the only writer.

**Default config:**
```json
{
  "items": [],
  "window_width": null,
  "window_height": null
}
```

---

## 8. Platform-Specific Considerations

| Feature | Windows | Linux | macOS |
|---|---|---|---|
| Icon extraction | `ExtractIconExW` via `windows` crate | `.desktop` file parsing + icon theme lookup | `.app` bundle `Info.plist` |
| `.lnk` shortcuts | `parselnk` crate | N/A (`.desktop` files) | N/A (`.app` bundles) |
| App data dir | `%APPDATA%` | `$XDG_CONFIG_HOME` or `~/.config` | `~/Library/Application Support` |
| Global hotkeys | `RegisterHotKey` (via `global-hotkey`) | X11/Wayland (via `global-hotkey`) | Carbon/CGEvent (via `global-hotkey`) |
| Open app | `ShellExecuteW` | `xdg-open` or direct `exec` | `NSWorkspace::openURL` |

The `global-hotkey` and `tray-icon` crates handle all cross-platform abstraction for hotkeys and tray. Only **icon extraction** and **app launching** need platform-specific code, isolated behind traits in `platform/`.

---

## 9. Risks and Technical Challenges

| Risk | Impact | Mitigation |
|---|---|---|
| **egui context menus not native** | Slightly different UX from Windows right-click | Acceptable — PowerToys Run also has custom menus. Build a clean custom popup. |
| **Icon extraction is complex on Windows** | Icons may be missing, wrong size, or low quality | Fall back to a generic app icon. Extract multiple sizes and pick the best. |
| **Global hotkey conflicts** | `Ctrl+Alt+R` may be taken by another app (e.g., AMD Radeon software) | Registration failure → show a warning dialog on first run. Future: make it configurable. |
| **egui on Windows with fractional scaling** | Blurry text on high-DPI displays | egui 0.30 handles this well; set `NativeOptions::centered` and query `pixels_per_point`. |
| **Config corruption** | App fails to start or loses data | Atomic writes (write-then-rename). Backup previous config on save. |
| **Opening a group that was deleted** | Crash or panic if nav stack references deleted item | When deleting a group, also clean the nav stack. Validate nav on render. |
| **.lnk file parsing** | `parselnk` might not parse all shortcut variants | Graceful error: show "Could not add shortcut" message. |

---

## 10. Phased Implementation Roadmap

### Phase 1 — Project Foundation
**Goal:** App runs in tray, hotkey shows/hides a window.

- `cargo init`, set up `Cargo.toml` with all dependencies
- `main.rs`: Initialize logging, config directories, system tray, global hotkey, egui
- `platform/tray.rs`: Create tray icon with "Show/Hide" and "Quit" menu
- `platform/hotkey.rs`: Register `Ctrl+Alt+R`, send on channel
- `app.rs`: Minimal `eframe::App` — toggle visibility on hotkey, close on Escape
- Window: always-on-top, frameless (or minimal decorations), centered on current monitor
- **Deliverable:** Run the app → icon in tray → `Ctrl+Alt+R` shows empty window → Esc hides it

### Phase 2 — Configuration & Data Model
**Goal:** Data structures defined, config persisted and loaded.

- `models/`: `ItemId`, `LaunchItem`, `AppItem`, `GroupItem`
- `config/manager.rs`: `ConfigManager` with `load()`, `save()`, `default()`
- Auto-create `%APPDATA%/LaunchPad/` on first run
- Atomic writes for `config.json`
- **Deliverable:** App starts with empty config; serialization round-trip verified with a test

### Phase 3 — Grid UI
**Goal:** Visual grid of apps and groups renders correctly.

- `ui/theme.rs`: Set up egui visuals — dark background, rounded cards, subtle shadows
- `ui/grid.rs`: Responsive grid layout (compute columns from window width)
- `ui/item_card.rs`: Render each item — icon (loaded from cache or default), title below
- `ui/launcher.rs`: Compose grid + navigation bar (back button when inside group)
- Navigation: click group → push to `nav_stack`; back button → pop
- Keyboard: arrow keys navigate, Enter opens selected item
- **Deliverable:** Add sample data in config → grid renders with icons and titles → groups open/close

### Phase 4 — Drag & Drop
**Goal:** Drop `.exe` or `.lnk` on window → auto-add to root.

- Enable egui file drop support
- Detect dropped file type (`.exe` → direct; `.lnk` → parse with `parselnk`)
- `platform/icons.rs`: Extract icon from executable using `ExtractIconExW`
- `commands/items.rs`: `add_app()` — create `AppItem`, derive title from filename
- Add to config root, persist, refresh UI
- **Deliverable:** Drag Chrome shortcut → appears in grid with icon and title

### Phase 5 — Context Menus
**Goal:** Right-click context menus for apps and groups.

- `ui/context_menu.rs`: Custom egui popup menu
- App menu: Open, Rename, Change Icon, Add to Group, Remove
- Group menu: Open, Rename, Change Icon, Delete
- Rename: inline text edit or simple dialog
- Change Icon: file picker (use `rfd` crate) → copy to icon cache → update config
- Remove: confirmation popup, then remove from config
- **Deliverable:** Right-click any item → menu appears → actions work and persist

### Phase 6 — Group Management
**Goal:** Full group CRUD + moving apps between groups.

- "Add to Group" in app context menu → submenu listing all groups + "New Group..."
- Create group: prompt for name → add `GroupItem` to root
- Delete group: confirmation → remove group and its apps from config
- Move app to group: remove from source, add to target group
- Group icon: custom or default folder icon
- **Deliverable:** Create "Games" group → move Steam + Discord into it → open group → see only those apps

### Phase 7 — Polish
**Goal:** Production-quality fit and finish.

- Keyboard shortcuts: `Ctrl+N` new group, `F2` rename, `Delete` remove selected
- Improved icon loading: show placeholder while loading, cache aggressively
- Error handling: non-critical errors shown as toast/status bar, not panics
- Config migration: handle old config formats gracefully
- Memory profiling: ensure idle RAM is under 20 MB
- Startup profiling: ensure cold start under 200 ms
- Window positioning: always centered on the monitor where the cursor is
- **Deliverable:** Feature-complete app, ready for daily use

---

## Summary of Key Decisions

1. **egui over Tauri** — RAM/startup win matters more for a launcher than native look
2. **Single process, synchronous** — no async runtime, keeps binary small and logic simple
3. **Config as single source of truth** — all state derives from it, auto-saved on mutation
4. **Window never destroyed** — toggled visible/hidden for instant appearance
5. **Groups are flat** — no nested groups (keeps complexity low; can revisit later)
6. **Platform isolation via traits** — only icon extraction and app launching need platform code

---

This covers all 10 deliverables from the prompt. Ready to start Phase 1 when you are. Would you like me to begin scaffolding the project?
