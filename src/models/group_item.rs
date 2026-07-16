use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{item::ItemId, item::LaunchItem};

/// A group (folder) containing application items.
///
/// Groups are intentionally flat — they only contain apps, not nested groups.
/// This keeps the UI and navigation simple.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupItem {
    /// Unique identifier.
    pub id: ItemId,
    /// Display title (e.g., "Games").
    pub title: String,
    /// Optional custom icon path. If None, a default folder icon is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<PathBuf>,
    /// The items contained in this group.
    #[serde(default)]
    pub items: Vec<LaunchItem>,
}

impl GroupItem {
    pub fn new(title: String) -> Self {
        Self {
            id: ItemId::new(),
            title,
            icon_path: None,
            items: Vec::new(),
        }
    }
}
