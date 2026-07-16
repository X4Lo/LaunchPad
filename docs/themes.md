# Launchpad — Themes

Themes let you customize the visual appearance of Launchpad.
They are stored in `config.json` and can be selected from the Settings menu.

## How themes work

1. Each theme is a set of **optional overrides** — any field left as `null` / omitted falls back to the built-in Default theme.
2. The `selected_theme` config field holds the **name** of the active theme.
3. `Config::resolve_theme()` looks up the name, merges its values over Default, and returns the effective theme.
4. If the selected theme name isn't found, the Default theme is used.

## Built-in themes

Launchpad ships with four themes:

| Name | Header | Body | Widgets | Accent |
|------|--------|------|---------|--------|
| **Default** | `#313337` | `#1F2127` | `#3A3C42` | `#606268` |
| **Dracula** | `#282A36` | `#21222C` | `#44475A` | `#6272A4` |
| **Nord** | `#2E3440` | `#242933` | `#3B4252` | `#5E81AC` |
| **Catppuccin** | `#1E1E2E` | `#181825` | `#313244` | `#CBA6F7` |

## Theme fields

All fields are optional (fall back to Default theme values):

```json
{
  "name": "My Theme",
  "header_color": "313337",
  "body_color": "1F2127",
  "widget_color": "3A3C42",
  "selection_color": "606268",
  "divider_color": "3D3F43",
  "text_color": null,
  "corner_radius": 12,
  "grid_spacing": 12.0,
  "grid_icon_size": 48.0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | **Required.** Unique name used to select the theme. |
| `header_color` | `String?` | 6-char hex for the title bar background. |
| `body_color` | `String?` | 6-char hex for the main window background. |
| `widget_color` | `String?` | 6-char hex for buttons and interactive elements. |
| `selection_color` | `String?` | 6-char hex for selection highlights. |
| `divider_color` | `String?` | 6-char hex for separator lines. |
| `text_color` | `String?` | 6-char hex for text (currently unused, reserved). |
| `corner_radius` | `u8?` | Window corner rounding in pixels (0–255). |
| `grid_spacing` | `f32?` | Gap between grid items in pixels. |
| `grid_icon_size` | `f32?` | Icon size in pixels. |

Colors use 6-character hex format without the `#` prefix (e.g., `"FF00AA"`).

## Creating a custom theme

1. Open `config.json` (in `%APPDATA%\Launchpad\` or next to the exe in portable mode).
2. Add your theme to the `"themes"` array:

```json
{
  "themes": [
    {
      "name": "Solarized Dark",
      "header_color": "002B36",
      "body_color": "073642",
      "widget_color": "586E75",
      "selection_color": "268BD2",
      "divider_color": "586E75",
      "corner_radius": 10,
      "grid_spacing": 12.0,
      "grid_icon_size": 48.0
    }
  ],
  "selected_theme": "Solarized Dark"
}
```

3. Set `"selected_theme"` to your theme's name.
4. Restart Launchpad (or toggle the theme in Settings → Themes).

Custom themes persist across updates — the built-in themes are merged in, but your custom themes are never overwritten.

## Selecting a theme

In the app: **Settings** (gear icon) → **Themes** → click a theme name.

The change takes effect immediately. Color swatches appear below the list to preview the selected theme's palette.
