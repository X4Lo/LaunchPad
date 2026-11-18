# Launchpad — Themes

Pick a theme in **Settings → Appearance**. The choice is stored as `selected_theme` in `config.json` (unset = Dark).

## Built-in themes

| Name | Background | Surface | Accent |
|------|-----------|---------|--------|
| **Dark** (default) | `#0d0f13` | `#161920` | `#7b6bff` |
| **Light** | `#f3f4f7` | `#ffffff` | `#6a59f5` |
| **Dracula** | `#21222c` | `#282a36` | `#bd93f9` |
| **Nord** | `#242933` | `#2e3440` | `#5e81ac` |
| **Catppuccin** (Mocha) | `#11111b` | `#1e1e2e` | `#cba6f7` |

Built-ins live in code (`src/ui/theme.rs`) and always appear in the list; they're no longer copied into `config.json`. Old configs that still contain copies named Default / Dracula / Nord / Catppuccin are cleaned up on load, and `"selected_theme": "Default"` means Dark.

## Custom themes

Add entries to `themes` in `config.json`; they appear under the built-ins in Settings. Every field except `name` is optional and falls back to Dark. Colors are 6-digit hex, with or without `#`.

```json
{
  "themes": [
    {
      "name": "Forest",
      "body_color": "1b2420",
      "header_color": "222e29",
      "widget_color": "2b3a33",
      "selection_color": "4caf7a",
      "divider_color": "2f3f37",
      "text_color": "e6efe9"
    }
  ],
  "selected_theme": "Forest"
}
```

| Field | Used for |
|-------|----------|
| `body_color` | Window background. Also decides light vs dark (title bar, text fallbacks). |
| `header_color` | Cards, menus and dialogs. |
| `widget_color` | Hover highlight. |
| `selection_color` | Accent: selection, switches, focus rings, buttons. |
| `divider_color` | Borders and separators. |
| `text_color` | Primary text (muted/faint text are derived from it). |

`corner_radius`, `grid_spacing` and `grid_icon_size` inside a theme are still accepted but ignored. Icon size and spacing are set in **Settings → Appearance → Layout** (`grid_icon_size`, `grid_spacing` at the top level of `config.json`).
