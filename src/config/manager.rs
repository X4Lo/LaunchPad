use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::item::LaunchItem;

// ─── Theme ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divider_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_spacing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_icon_size: Option<f32>,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            name: "Default".into(),
            header_color: Some("313337".into()),
            body_color: Some("1F2127".into()),
            widget_color: Some("3A3C42".into()),
            selection_color: Some("606268".into()),
            divider_color: Some("3D3F43".into()),
            text_color: None,
            corner_radius: Some(12),
            grid_spacing: Some(12.0),
            grid_icon_size: Some(48.0),
        }
    }

    /// Built-in themes shipped with the app.
    pub fn builtin_themes() -> Vec<Theme> {
        vec![
            Self::default_theme(),
            Theme {
                name: "Dracula".into(),
                header_color: Some("282A36".into()),
                body_color: Some("21222C".into()),
                widget_color: Some("44475A".into()),
                selection_color: Some("6272A4".into()),
                divider_color: Some("44475A".into()),
                text_color: Some("F8F8F2".into()),
                corner_radius: Some(10),
                grid_spacing: Some(12.0),
                grid_icon_size: Some(48.0),
            },
            Theme {
                name: "Nord".into(),
                header_color: Some("2E3440".into()),
                body_color: Some("242933".into()),
                widget_color: Some("3B4252".into()),
                selection_color: Some("5E81AC".into()),
                divider_color: Some("3B4252".into()),
                text_color: Some("D8DEE9".into()),
                corner_radius: Some(8),
                grid_spacing: Some(10.0),
                grid_icon_size: Some(48.0),
            },
            Theme {
                name: "Catppuccin".into(),
                header_color: Some("1E1E2E".into()),
                body_color: Some("181825".into()),
                widget_color: Some("313244".into()),
                selection_color: Some("CBA6F7".into()),
                divider_color: Some("313244".into()),
                text_color: Some("CDD6F4".into()),
                corner_radius: Some(14),
                grid_spacing: Some(14.0),
                grid_icon_size: Some(52.0),
            },
        ]
    }

    /// Parse a hex string like "313337" or "FF00AA" into Color32.
    pub fn parse_hex(s: &str) -> Option<egui::Color32> {
        let s = s.trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(egui::Color32::from_rgb(r, g, b))
    }
}

// ─── Config ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub items: Vec<LaunchItem>,

    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default)]
    pub window_x: Option<f32>,
    #[serde(default)]
    pub window_y: Option<f32>,

    #[serde(default = "default_spacing")]
    pub grid_spacing: f32,
    #[serde(default = "default_icon_size")]
    pub grid_icon_size: f32,

    /// Whether to hide the launcher after launching an app.
    #[serde(default)]
    pub hide_on_launch: bool,

    /// Available themes.
    #[serde(default = "Theme::builtin_themes")]
    pub themes: Vec<Theme>,

    /// Name of the currently selected theme (None = use defaults).
    #[serde(default)]
    pub selected_theme: Option<String>,

    /// Global hotkey string, e.g. "Ctrl+Alt+R".
    #[serde(default = "default_hotkey")]
    pub hotkey: String,

    /// Whether the hotkey triggers on key release (true) or key press (false).
    #[serde(default = "default_true")]
    pub hotkey_on_release: bool,

    /// Launchpad should start automatically with Windows.
    #[serde(default)]
    pub auto_start: bool,
}

fn default_spacing() -> f32 {
    12.0
}
fn default_icon_size() -> f32 {
    48.0
}
fn default_hotkey() -> String {
    "Ctrl+Alt+R".into()
}
fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            grid_spacing: default_spacing(),
            grid_icon_size: default_icon_size(),
            hide_on_launch: false,
            themes: Theme::builtin_themes(),
            selected_theme: None,
            hotkey: default_hotkey(),
            hotkey_on_release: true,
            auto_start: false,
        }
    }
}

impl Config {
    /// Get the effective theme: the selected theme merged over the default.
    pub fn resolve_theme(&self) -> Theme {
        let default = Theme::default_theme();
        if let Some(ref name) = self.selected_theme {
            if let Some(t) = self
                .themes
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(name))
            {
                return Theme {
                    name: t.name.clone(),
                    header_color: t.header_color.clone().or(default.header_color),
                    body_color: t.body_color.clone().or(default.body_color),
                    widget_color: t.widget_color.clone().or(default.widget_color),
                    selection_color: t.selection_color.clone().or(default.selection_color),
                    divider_color: t.divider_color.clone().or(default.divider_color),
                    text_color: t.text_color.clone().or(default.text_color),
                    corner_radius: t.corner_radius.or(default.corner_radius),
                    grid_spacing: t.grid_spacing.or(default.grid_spacing),
                    grid_icon_size: t.grid_icon_size.or(default.grid_icon_size),
                };
            }
        }
        default
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    /// Get the Launchpad data directory (creates it if needed).
    pub fn data_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Launchpad")
    }

    /// Get the path to a portable config file next to the executable (if it exists).
    fn portable_config_path() -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        let candidate = exe_dir.join("config.json");
        if candidate.exists() {
            log::info!("Using portable config: {}", candidate.display());
            Some(candidate)
        } else {
            None
        }
    }

    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Portable mode: if config.json exists next to the exe, use it.
        if let Some(portable) = Self::portable_config_path() {
            return Ok(Self {
                config_path: portable,
            });
        }
        let data_dir = dirs::config_dir()
            .ok_or("Could not determine config directory")?
            .join("Launchpad");
        std::fs::create_dir_all(&data_dir)?;
        Ok(Self {
            config_path: data_dir.join("config.json"),
        })
    }

    /// Returns the icons directory (next to the config file).
    pub fn icons_dir(&self) -> PathBuf {
        self.config_path
            .parent()
            .map(|p| p.join("icons"))
            .unwrap_or_else(|| PathBuf::from("icons"))
    }

    pub fn load(&self) -> Result<Config, Box<dyn std::error::Error>> {
        if !self.config_path.exists() {
            let default_config = Config::default();
            self.save(&default_config)?;
            return Ok(default_config);
        }
        let contents = std::fs::read_to_string(&self.config_path)?;
        let mut config: Config = serde_json::from_str(&contents).unwrap_or_else(|e| {
            log::warn!("Failed to parse config, using default: {}", e);
            Config::default()
        });
        // Merge built-in themes: add any that don't exist yet in the user's config
        for builtin in Theme::builtin_themes() {
            if !config
                .themes
                .iter()
                .any(|t| t.name.eq_ignore_ascii_case(&builtin.name))
            {
                config.themes.push(builtin);
            }
        }
        Ok(config)
    }

    pub fn save(&self, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(config)?;
        let tmp_path = self.config_path.with_extension("tmp");
        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, &self.config_path)?;
        Ok(())
    }
}
