use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::item::ItemId;

/// A folder launch item — opens a directory in the file manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderItem {
    /// Unique identifier.
    pub id: ItemId,
    /// Display title (e.g., "Projects").
    pub title: String,
    /// Optional custom icon path. If None, a default folder icon is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<PathBuf>,
    /// Path to the folder on disk.
    pub folder_path: PathBuf,
}

impl FolderItem {
    pub fn new(title: String, folder_path: PathBuf) -> Self {
        Self {
            id: ItemId::new(),
            title,
            icon_path: None,
            folder_path,
        }
    }
}
