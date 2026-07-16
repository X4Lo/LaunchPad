use std::path::PathBuf;

use crate::config::manager::Config;
use crate::models::app_item::AppItem;
use crate::models::folder_item::FolderItem;
use crate::models::group_item::GroupItem;
use crate::models::item::{ItemId, LaunchItem};

/// Add a new application to the root level of the config.
pub fn add_app(config: &mut Config, title: String, executable_path: PathBuf) -> ItemId {
    let app = AppItem::new(title, executable_path);
    let id = app.id;
    config.items.push(LaunchItem::App(app));
    log::info!(
        "Added app: {} (id: {})",
        config.items.last().unwrap().title(),
        id
    );
    id
}

/// Add a new application to a specific group.
pub fn add_app_to_group(
    config: &mut Config,
    group_id: ItemId,
    title: String,
    executable_path: PathBuf,
) -> Result<ItemId, String> {
    let group = find_group_mut(config, group_id).ok_or("Group not found")?;
    let app = AppItem::new(title, executable_path);
    let id = app.id;
    group.items.push(LaunchItem::App(app));
    Ok(id)
}

/// Create a new group at the root level.
pub fn add_group(config: &mut Config, title: String) -> ItemId {
    let group = GroupItem::new(title);
    let id = group.id;
    config.items.push(LaunchItem::Group(group));
    log::info!(
        "Added group: {} (id: {})",
        config.items.last().unwrap().title(),
        id
    );
    id
}

/// Add a new folder to the root level of the config.
pub fn add_folder(config: &mut Config, title: String, folder_path: PathBuf) -> ItemId {
    let folder = FolderItem::new(title, folder_path);
    let id = folder.id;
    config.items.push(LaunchItem::Folder(folder));
    log::info!(
        "Added folder: {} (id: {})",
        config.items.last().unwrap().title(),
        id
    );
    id
}

/// Add a new folder to a specific group.
pub fn add_folder_to_group(
    config: &mut Config,
    group_id: ItemId,
    title: String,
    folder_path: PathBuf,
) -> Result<ItemId, String> {
    let group = find_group_mut(config, group_id).ok_or("Group not found")?;
    let folder = FolderItem::new(title, folder_path);
    let id = folder.id;
    group.items.push(LaunchItem::Folder(folder));
    Ok(id)
}

/// Remove an item by ID from the root level or any group.
pub fn remove_item(config: &mut Config, id: ItemId) -> Result<(), String> {
    // Try removing from root
    if let Some(pos) = config.items.iter().position(|i| i.id() == id) {
        let removed = config.items.remove(pos);
        log::info!("Removed item from root: {} (id: {})", removed.title(), id);
        return Ok(());
    }

    // Search in groups
    for group in config.items.iter_mut() {
        if let LaunchItem::Group(g) = group {
            if let Some(pos) = g.items.iter().position(|i| i.id() == id) {
                let removed = g.items.remove(pos);
                log::info!(
                    "Removed item from group '{}': {} (id: {})",
                    g.title,
                    removed.title(),
                    id
                );
                return Ok(());
            }
        }
    }

    Err(format!("Item with id {} not found", id))
}

/// Rename any item (app or group) by ID.
pub fn rename_item(config: &mut Config, id: ItemId, new_title: String) -> Result<(), String> {
    // Search root level
    for item in config.items.iter_mut() {
        if let Some(renamed) = rename_if_match(item, id, &new_title) {
            log::info!("Renamed root item: {} -> {}", renamed, new_title);
            return Ok(());
        }
    }

    // Search in groups
    for item in config.items.iter_mut() {
        if let LaunchItem::Group(g) = item {
            for sub in g.items.iter_mut() {
                if let Some(renamed) = rename_if_match(sub, id, &new_title) {
                    log::info!(
                        "Renamed item in group '{}': {} -> {}",
                        g.title,
                        renamed,
                        new_title
                    );
                    return Ok(());
                }
            }
        }
    }

    Err(format!("Item with id {} not found", id))
}

/// Set a custom icon path for an item.
pub fn set_icon(config: &mut Config, id: ItemId, icon_path: PathBuf) -> Result<(), String> {
    for item in config.items.iter_mut() {
        if set_icon_if_match(item, id, &icon_path) {
            return Ok(());
        }
    }
    for item in config.items.iter_mut() {
        if let LaunchItem::Group(g) = item {
            for sub in g.items.iter_mut() {
                if set_icon_if_match(sub, id, &icon_path) {
                    return Ok(());
                }
            }
        }
    }
    Err(format!("Item with id {} not found", id))
}

/// Clear the custom icon for an item (revert to default).
pub fn clear_icon(config: &mut Config, id: ItemId) -> Result<(), String> {
    for item in config.items.iter_mut() {
        if clear_icon_if_match(item, id) {
            return Ok(());
        }
    }
    for item in config.items.iter_mut() {
        if let LaunchItem::Group(g) = item {
            for sub in g.items.iter_mut() {
                if clear_icon_if_match(sub, id) {
                    return Ok(());
                }
            }
        }
    }
    Err(format!("Item with id {} not found", id))
}

/// Move an app from its current location into a group.
pub fn move_to_group(config: &mut Config, item_id: ItemId, group_id: ItemId) -> Result<(), String> {
    // Find and remove the item
    let item = remove_from_anywhere(config, item_id)
        .ok_or_else(|| format!("Item with id {} not found", item_id))?;

    // Ensure it's an App, not a Group or Folder
    if matches!(&item, LaunchItem::Group(_) | LaunchItem::Folder(_)) {
        return Err("Cannot move a group or folder into another group".into());
    }

    // Find the target group and add the item
    let group = find_group_mut(config, group_id).ok_or("Target group not found")?;
    group.items.push(item);
    log::info!("Moved item {} to group {}", item_id, group_id);
    Ok(())
}

/// Delete a group and all its contents.
pub fn delete_group(config: &mut Config, group_id: ItemId) -> Result<(), String> {
    if let Some(pos) = config.items.iter().position(|i| i.id() == group_id) {
        if matches!(&config.items[pos], LaunchItem::Group(_)) {
            let removed = config.items.remove(pos);
            log::info!("Deleted group: {} (id: {})", removed.title(), group_id);
            Ok(())
        } else {
            Err("Item is not a group".into())
        }
    } else {
        Err(format!("Group with id {} not found", group_id))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn rename_if_match(item: &mut LaunchItem, id: ItemId, new_title: &str) -> Option<String> {
    let old = item.title().to_owned();
    match item {
        LaunchItem::App(app) if app.id == id => {
            app.title = new_title.to_owned();
            Some(old)
        }
        LaunchItem::Group(group) if group.id == id => {
            group.title = new_title.to_owned();
            Some(old)
        }
        LaunchItem::Folder(folder) if folder.id == id => {
            folder.title = new_title.to_owned();
            Some(old)
        }
        _ => None,
    }
}

fn set_icon_if_match(item: &mut LaunchItem, id: ItemId, path: &PathBuf) -> bool {
    match item {
        LaunchItem::App(app) if app.id == id => {
            app.icon_path = Some(path.clone());
            true
        }
        LaunchItem::Group(group) if group.id == id => {
            group.icon_path = Some(path.clone());
            true
        }
        LaunchItem::Folder(folder) if folder.id == id => {
            folder.icon_path = Some(path.clone());
            true
        }
        _ => false,
    }
}

fn clear_icon_if_match(item: &mut LaunchItem, id: ItemId) -> bool {
    match item {
        LaunchItem::App(app) if app.id == id => {
            app.icon_path = None;
            true
        }
        LaunchItem::Group(group) if group.id == id => {
            group.icon_path = None;
            true
        }
        LaunchItem::Folder(folder) if folder.id == id => {
            folder.icon_path = None;
            true
        }
        _ => false,
    }
}

fn find_group_mut(config: &mut Config, group_id: ItemId) -> Option<&mut GroupItem> {
    config.items.iter_mut().find_map(|item| {
        if let LaunchItem::Group(g) = item {
            if g.id == group_id {
                Some(g)
            } else {
                None
            }
        } else {
            None
        }
    })
}

fn remove_from_anywhere(config: &mut Config, id: ItemId) -> Option<LaunchItem> {
    if let Some(pos) = config.items.iter().position(|i| i.id() == id) {
        return Some(config.items.remove(pos));
    }
    for item in config.items.iter_mut() {
        if let LaunchItem::Group(g) = item {
            if let Some(pos) = g.items.iter().position(|i| i.id() == id) {
                return Some(g.items.remove(pos));
            }
        }
        // Folders don't contain sub-items, so nothing to search inside them.
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_add_and_remove_app() {
        let mut cfg = make_config();
        let id = add_app(&mut cfg, "Firefox".into(), PathBuf::from("C:/firefox.exe"));
        assert_eq!(cfg.items.len(), 1);
        assert_eq!(cfg.items[0].title(), "Firefox");

        remove_item(&mut cfg, id).unwrap();
        assert!(cfg.items.is_empty());
    }

    #[test]
    fn test_add_and_delete_group() {
        let mut cfg = make_config();
        let gid = add_group(&mut cfg, "Games".into());
        assert_eq!(cfg.items.len(), 1);
        assert_eq!(cfg.items[0].title(), "Games");

        delete_group(&mut cfg, gid).unwrap();
        assert!(cfg.items.is_empty());
    }

    #[test]
    fn test_move_app_to_group() {
        let mut cfg = make_config();
        let app_id = add_app(&mut cfg, "Steam".into(), PathBuf::from("C:/steam.exe"));
        let group_id = add_group(&mut cfg, "Games".into());

        move_to_group(&mut cfg, app_id, group_id).unwrap();

        // Root should only have the group
        assert_eq!(cfg.items.len(), 1);
        assert!(matches!(&cfg.items[0], LaunchItem::Group(_)));

        // App should be inside the group
        if let LaunchItem::Group(g) = &cfg.items[0] {
            assert_eq!(g.items.len(), 1);
            assert_eq!(g.items[0].title(), "Steam");
        } else {
            panic!("Expected group");
        }
    }

    #[test]
    fn test_rename_item() {
        let mut cfg = make_config();
        let id = add_app(&mut cfg, "Chrome".into(), PathBuf::from("C:/chrome.exe"));

        rename_item(&mut cfg, id, "Google Chrome".into()).unwrap();
        assert_eq!(cfg.items[0].title(), "Google Chrome");
    }

    #[test]
    fn test_cannot_move_group_into_group() {
        let mut cfg = make_config();
        let g1 = add_group(&mut cfg, "Group 1".into());
        let g2 = add_group(&mut cfg, "Group 2".into());

        let result = move_to_group(&mut cfg, g1, g2);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut cfg = make_config();
        let fake_id = ItemId::new();
        assert!(remove_item(&mut cfg, fake_id).is_err());
    }

    #[test]
    fn test_config_json_roundtrip() {
        let mut cfg = make_config();
        add_app(
            &mut cfg,
            "Notepad".into(),
            PathBuf::from("C:/Windows/notepad.exe"),
        );
        add_group(&mut cfg, "Utilities".into());

        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].title(), "Notepad");
        assert_eq!(parsed.items[1].title(), "Utilities");
    }
}
