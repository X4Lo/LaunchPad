use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::item::ItemId;

/// An application launch item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppItem {
    /// Unique identifier.
    pub id: ItemId,
    /// Display title (e.g., "Google Chrome").
    pub title: String,
    /// Optional custom icon path. If None, the system icon is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<PathBuf>,
    /// Path to the executable.
    pub executable_path: PathBuf,
}

impl AppItem {
    pub fn new(title: String, executable_path: PathBuf) -> Self {
        Self {
            id: ItemId::new(),
            title,
            icon_path: None,
            executable_path,
        }
    }
}
