# AI Agent Prompt - Launchpad (Rust Desktop Application)

You are a senior Rust desktop application architect.

I want you to help me build a desktop application called **Launchpad**.

Your first task is **NOT** to write code.

Instead, analyze the project, design the architecture, identify the best libraries, and produce a phased implementation plan. We will then implement each phase one at a time.

---

# Project Overview

Launchpad is a very lightweight desktop application whose purpose is to act as a launcher for all applications I use daily.

Instead of having shortcuts scattered across the desktop or taskbar, I want one centralized launcher.

The application should remain minimalistic, fast, and lightweight.

---

# Core Behavior

The application should run in the system tray.

The user can assign a global shortcut (initially hardcoded).

Example:

```
Ctrl + Alt + R
```

When the shortcut is pressed:

- Launchpad appears instantly
- It is always on top
- It gains focus
- It opens centered on the current monitor
- Pressing Escape closes it
- Clicking outside also closes it

Think of it somewhat like macOS Launchpad or Spotlight, but much simpler.

---

# Main UI

The main window should display a grid.

Each grid item represents either:

- an application
- or a group (folder)

Each item should display:

- icon
- title underneath

Example:

```
Chrome      VSCode      Steam

Games       Discord     Spotify

Work        Docker      Terminal
```

Groups behave like folders.

Opening a group displays another grid containing only the apps inside that group.

Navigation should be simple.

---

# Drag & Drop

The user should be able to drag an executable, desktop shortcut, or application into Launchpad.

When dropped:

- detect the executable
- retrieve its icon
- retrieve a reasonable title
- automatically add it to the root collection

---

# Groups

Groups are folders.

The user can:

- create group
- rename group
- delete group
- assign custom icon
- open group

Groups only contain applications (not nested groups unless you believe supporting them adds little complexity—discuss the trade-offs before implementing).

---

# Context Menu

Right-clicking an application should display a context menu.

Initially include:

- Open
- Rename
- Change Icon
- Add to Group
- Remove

Right-clicking a group:

- Open
- Rename
- Change Icon
- Delete

---

# Configuration

Everything should be persisted in a JSON configuration file.

Suggested structure:

```text
config.json

items
    App
    App
    Group
    App
```

Apps:

```json
{
  "type": "app",
  "title": "Google Chrome",
  "iconPath": "...",
  "path": "C:\\Program Files\\Google\\Chrome\\chrome.exe"
}
```

Groups:

```json
{
  "type": "group",
  "title": "Games",
  "iconPath": "...",
  "apps": [
      ...
  ]
}
```

By default:

Applications use their system icon.

Groups use a default folder icon.

Custom icons should override defaults.

---

# Data Model

Application:

- title
- iconPath
- path

Group:

- title
- iconPath
- list of applications

---

# Non-Goals

Do NOT build:

- search
- cloud sync
- plugins
- themes
- databases
- networking
- telemetry

Keep everything local.

---

# Performance Goals

The application should:

- launch instantly
- use very little RAM
- stay responsive
- have almost zero CPU usage while idle

---

# Technology

Language:

Rust

Please recommend the best GUI framework after evaluating:

- egui/eframe
- iced
- Tauri (desktop mode)
- Slint
- Dioxus Desktop
- native-windows-gui (Windows only)

The application is Windows-first, but I'd appreciate notes on portability.

Also recommend crates for:

- global hotkeys
- system tray
- drag-and-drop
- executable icon extraction
- JSON serialization
- filesystem watching (if needed)

---

# UI Style

Very minimal.

Modern.

Flat.

Rounded corners.

No unnecessary animations.

Small shadows are acceptable.

The interface should feel similar to:

- PowerToys Run
- Raycast
- macOS Launchpad

while remaining extremely simple.

---

# Suggested Project Structure

Please propose a clean Rust project structure.

Example:

```
src/

app/
config/
models/
ui/
widgets/
icons/
commands/
storage/
platform/
utils/
main.rs
```

Feel free to improve this.

---

# Development Philosophy

Implement features in small, reviewable phases.

Each phase should produce a working application.

Avoid large rewrites later.

Favor maintainability over cleverness.

---

# Deliverables (Planning Phase)

Before writing any code, provide:

1. High-level architecture.
2. Recommended GUI framework and justification.
3. Crate recommendations.
4. Proposed folder structure.
5. Data model.
6. State management approach.
7. Configuration management.
8. Platform-specific considerations.
9. Risks and technical challenges.
10. A phased implementation roadmap.

---

# Proposed Implementation Phases

## Phase 1 – Project Foundation

- Create the Rust project.
- Set up the chosen GUI framework.
- Implement the main window.
- Add a system tray icon.
- Register the global hotkey (`Ctrl + Alt + R`).
- Show/hide the launcher.
- Keep the window always on top.
- Close on `Esc` or loss of focus.

## Phase 2 – Configuration & Data Model

- Define the `App` and `Group` models.
- Implement JSON serialization/deserialization with `serde`.
- Load configuration on startup.
- Save changes automatically.
- Create default configuration if none exists.

## Phase 3 – Grid UI

- Render a responsive grid of apps and groups.
- Display application icons and titles.
- Display folder icons and titles.
- Handle selection and hover states.
- Support opening groups and navigating back.

## Phase 4 – Drag & Drop

- Accept dropped executables and shortcuts.
- Extract application metadata (title, path, icon).
- Add new apps to the root collection.
- Persist changes to the configuration.

## Phase 5 – Context Menus

- Right-click context menus for apps and groups.
- Rename items.
- Remove items.
- Change icons.
- Move apps into groups.

## Phase 6 – Group Management

- Create and delete groups.
- Assign custom icons.
- Move apps between groups.
- Ensure configuration updates correctly.

## Phase 7 – Polish

- Improve keyboard navigation.
- Refine layout and spacing.
- Add confirmation dialogs where appropriate.
- Improve startup time and memory usage.
- Handle edge cases and error reporting gracefully.

---

Build the application incrementally. Do not skip ahead or introduce features outside the defined scope unless explicitly discussed first. At the start of each implementation phase, briefly explain the approach and any trade-offs before writing code.
