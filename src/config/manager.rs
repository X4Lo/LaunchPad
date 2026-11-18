use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::item::LaunchItem;

// ─── Custom theme ────────────────────────────────────────

/// A user-defined colour theme from `config.json`.
///
/// Built-in themes (Dark, Light, Dracula, Nord, Catppuccin) live in code
/// (`ui::theme`); this struct only describes *custom* ones. Every field is optional
/// and falls back to the Dark theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divider_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    /// Legacy fields from the egui UI. Kept so old configs round-trip; no longer used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_spacing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_icon_size: Option<f32>,
}

/// Parse a hex colour like `"313337"` or `"#FF00AA"`.
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Expand `$(Drive)` (the portable-launcher convention for "the drive this app runs
/// from", e.g. `C:`) in a stored path. Other paths are returned unchanged.
pub fn expand_path(path: &Path) -> PathBuf {
    const VAR: &str = "$(drive)";
    let s = path.to_string_lossy();
    let Some(at) = s.to_ascii_lowercase().find(VAR) else { return path.to_path_buf() };
    let drive = std::env::current_exe()
        .ok()
        .and_then(|e| e.to_str().and_then(|s| s.get(..2)).filter(|d| d.ends_with(':')).map(str::to_owned))
        .unwrap_or_else(|| "C:".into());
    PathBuf::from(format!("{}{drive}{}", &s[..at], &s[at + VAR.len()..]))
}

/// Names the egui version wrote into `config.json` as copies of its built-ins.
/// They are dropped on load: the built-ins now live in code.
const LEGACY_BUILTIN_THEMES: [&str; 4] = ["Default", "Dracula", "Nord", "Catppuccin"];

// ─── Config ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub items: Vec<LaunchItem>,

    /// Window geometry in logical pixels.
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
    /// Grow or shrink icons so every item fits the window.
    #[serde(default)]
    pub auto_fit: bool,

    /// Hide the launcher after launching an app or opening a folder.
    #[serde(default)]
    pub hide_on_launch: bool,

    /// Custom themes (built-ins are always available).
    #[serde(default)]
    pub themes: Vec<Theme>,

    /// Name of the active theme. `None` = Dark.
    #[serde(default)]
    pub selected_theme: Option<String>,

    /// Global hotkey string, e.g. "Ctrl+Alt+R".
    #[serde(default = "default_hotkey")]
    pub hotkey: String,

    /// Whether the hotkey triggers on key release (true) or key press (false).
    #[serde(default = "default_true")]
    pub hotkey_on_release: bool,

    /// Launchpad starts automatically (in the tray) when you sign in.
    #[serde(default)]
    pub auto_start: bool,

    /// Closing the window hides it to the tray instead of quitting.
    #[serde(default)]
    pub close_to_tray: bool,

    /// Keep the launcher above other windows.
    #[serde(default)]
    pub always_on_top: bool,
}

fn default_spacing() -> f32 {
    12.0
}
fn default_icon_size() -> f32 {
    56.0
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
            auto_fit: false,
            hide_on_launch: false,
            themes: Vec::new(),
            selected_theme: None,
            hotkey: default_hotkey(),
            hotkey_on_release: true,
            auto_start: false,
            close_to_tray: false,
            always_on_top: false,
        }
    }
}

impl Config {
    /// Clamp values a hand-edited or older config may have out of range.
    fn sanitize(&mut self) {
        self.themes.retain(|t| !LEGACY_BUILTIN_THEMES.iter().any(|b| t.name.eq_ignore_ascii_case(b)));
        if self.selected_theme.as_deref().is_some_and(|n| n.eq_ignore_ascii_case("Default")) {
            self.selected_theme = None;
        }
        if !self.grid_icon_size.is_finite() {
            self.grid_icon_size = default_icon_size();
        }
        if !self.grid_spacing.is_finite() {
            self.grid_spacing = default_spacing();
        }
        self.grid_icon_size = self.grid_icon_size.clamp(32.0, 128.0);
        self.grid_spacing = self.grid_spacing.clamp(0.0, 40.0);
    }
}

// ─── ConfigManager ───────────────────────────────────────

pub struct ConfigManager {
    config_path: PathBuf,
    /// true when no config file exists anywhere — app should show first-run setup.
    pub pending_first_run: bool,
}

impl ConfigManager {
    fn appdata_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("Launchpad"))
    }

    fn exe_dir() -> Option<PathBuf> {
        std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
    }

    /// Get the path to a portable config next to the exe, if it exists.
    fn portable_config_path() -> Option<PathBuf> {
        let candidate = Self::exe_dir()?.join("config.json");
        candidate.exists().then(|| {
            log::info!("Using portable config: {}", candidate.display());
            candidate
        })
    }

    /// Resolve the config path. When no config file exists, `pending_first_run` is set
    /// and the app asks the user where to store it before anything is written.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(portable) = Self::portable_config_path() {
            return Ok(Self { config_path: portable, pending_first_run: false });
        }
        let appdata_dir = Self::appdata_dir().ok_or("Could not determine config directory")?;
        let appdata_path = appdata_dir.join("config.json");
        if appdata_path.exists() {
            return Ok(Self { config_path: appdata_path, pending_first_run: false });
        }
        log::info!("First run detected — deferring setup to in-app dialog");
        Ok(Self { config_path: appdata_path, pending_first_run: true })
    }

    pub fn path(&self) -> &Path {
        &self.config_path
    }

    /// Called once the user chooses portable or normal mode. Falls back to AppData if
    /// the exe's folder isn't writable. Returns whether portable mode is in effect.
    pub fn finalize_first_run(&mut self, use_portable: bool, config: &Config) -> Result<bool, String> {
        if use_portable {
            if let Some(dir) = Self::exe_dir() {
                let path = dir.join("config.json");
                let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
                if std::fs::write(&path, json).is_ok() {
                    log::info!("Portable config created at: {}", path.display());
                    self.config_path = path;
                    self.pending_first_run = false;
                    return Ok(true);
                }
            }
            log::warn!("Portable config creation failed, falling back to AppData");
        }
        let dir = Self::appdata_dir().ok_or("Could not determine config directory")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        self.config_path = dir.join("config.json");
        self.pending_first_run = false;
        self.save(config).map_err(|e| e.to_string())?;
        Ok(false)
    }

    /// The icons directory (next to the config file).
    pub fn icons_dir(&self) -> PathBuf {
        self.config_path.parent().map(|p| p.join("icons")).unwrap_or_else(|| PathBuf::from("icons"))
    }

    /// Resolve a stored icon path: bare filenames live in the icons directory.
    pub fn resolve_icon(&self, path: &Path) -> PathBuf {
        if path.parent().is_none_or(|p| p.as_os_str().is_empty()) {
            self.icons_dir().join(path)
        } else {
            expand_path(path)
        }
    }

    /// Load config from disk. If pending first run, returns defaults without saving.
    pub fn load(&self) -> Config {
        if self.pending_first_run || !self.config_path.exists() {
            return Config::default();
        }
        let mut config: Config = match std::fs::read_to_string(&self.config_path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                log::warn!("Failed to parse config, using defaults: {e}");
                Config::default()
            }),
            Err(e) => {
                log::warn!("Failed to read config, using defaults: {e}");
                Config::default()
            }
        };
        config.sanitize();

        // Sync auto-start with the real registry state (the user may have removed it).
        let actual = crate::platform::autostart::is_enabled();
        if config.auto_start != actual {
            log::info!("Auto-start mismatch: config={}, system={actual}. Syncing.", config.auto_start);
            config.auto_start = actual;
        }
        config
    }

    pub fn save(&self, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        if self.pending_first_run {
            return Ok(());
        }
        let json = serde_json::to_string_pretty(config)?;
        let tmp_path = self.config_path.with_extension("tmp");
        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, &self.config_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("313337"), Some((0x31, 0x33, 0x37)));
        assert_eq!(parse_hex("#ff00AA"), Some((0xff, 0x00, 0xaa)));
        assert_eq!(parse_hex("12345"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
    }

    #[test]
    fn drive_variable_expands_to_a_drive() {
        let p = expand_path(Path::new(r"$(Drive)\Program Files\App\app.exe"));
        let s = p.to_string_lossy();
        assert!(!s.contains("$("), "{s}");
        assert!(s.ends_with(r":\Program Files\App\app.exe"), "{s}");
        assert_eq!(expand_path(Path::new(r"D:\x.exe")), PathBuf::from(r"D:\x.exe"));
    }

    #[test]
    fn legacy_config_loads_and_drops_builtin_copies() {
        let json = r#"{
            "items": [],
            "themes": [{"name": "Default", "body_color": "1F2127"}, {"name": "Mine", "body_color": "101010"}],
            "selected_theme": "Default",
            "grid_icon_size": 500
        }"#;
        let mut cfg: Config = serde_json::from_str(json).unwrap();
        cfg.sanitize();
        assert_eq!(cfg.themes.len(), 1);
        assert_eq!(cfg.themes[0].name, "Mine");
        assert_eq!(cfg.selected_theme, None);
        assert_eq!(cfg.grid_icon_size, 128.0);
        assert!(cfg.hotkey_on_release, "missing field uses its default");
    }
}
