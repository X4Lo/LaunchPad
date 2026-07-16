use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a launch item.
///
/// Wraps a UUID v4 to provide stable identity across renames and moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ItemId(pub Uuid);

impl ItemId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A launch item: an app, a group, or a folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LaunchItem {
    #[serde(rename = "app")]
    App(crate::models::app_item::AppItem),
    #[serde(rename = "group")]
    Group(crate::models::group_item::GroupItem),
    #[serde(rename = "folder")]
    Folder(crate::models::folder_item::FolderItem),
}

impl LaunchItem {
    /// Get the display title of this item.
    pub fn title(&self) -> &str {
        match self {
            LaunchItem::App(app) => &app.title,
            LaunchItem::Group(group) => &group.title,
            LaunchItem::Folder(folder) => &folder.title,
        }
    }

    /// Get the unique ID of this item.
    pub fn id(&self) -> ItemId {
        match self {
            LaunchItem::App(app) => app.id,
            LaunchItem::Group(group) => group.id,
            LaunchItem::Folder(folder) => folder.id,
        }
    }

    /// Get a mutable reference to the inner GroupItem, if this is a Group.
    pub fn as_group_mut(&mut self) -> Option<&mut crate::models::group_item::GroupItem> {
        match self {
            LaunchItem::Group(g) => Some(g),
            _ => None,
        }
    }
}
