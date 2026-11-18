//! Mutations of the item tree. Groups are one level deep: the root holds apps, folders
//! and groups; a group holds apps and folders.

use std::path::PathBuf;

use crate::config::manager::Config;
use crate::models::app_item::AppItem;
use crate::models::folder_item::FolderItem;
use crate::models::group_item::GroupItem;
use crate::models::item::{ItemId, LaunchItem};

/// Where an item lives: the root (`None`) or inside a group.
pub type Parent = Option<ItemId>;

/// The item list of `parent`, or `None` if that group doesn't exist.
pub fn list(config: &Config, parent: Parent) -> Option<&Vec<LaunchItem>> {
    match parent {
        None => Some(&config.items),
        Some(gid) => config.items.iter().find_map(|i| match i {
            LaunchItem::Group(g) if g.id == gid => Some(&g.items),
            _ => None,
        }),
    }
}

fn list_mut(config: &mut Config, parent: Parent) -> Option<&mut Vec<LaunchItem>> {
    match parent {
        None => Some(&mut config.items),
        Some(gid) => find_group_mut(config, gid).map(|g| &mut g.items),
    }
}

/// Find an item anywhere, with the group it lives in.
pub fn find(config: &Config, id: ItemId) -> Option<(&LaunchItem, Parent)> {
    for item in &config.items {
        if item.id() == id {
            return Some((item, None));
        }
        if let LaunchItem::Group(g) = item {
            if let Some(sub) = g.items.iter().find(|s| s.id() == id) {
                return Some((sub, Some(g.id)));
            }
        }
    }
    None
}

fn find_mut(config: &mut Config, id: ItemId) -> Option<&mut LaunchItem> {
    for item in config.items.iter_mut() {
        if item.id() == id {
            return Some(item);
        }
        if let LaunchItem::Group(g) = item {
            if let Some(sub) = g.items.iter_mut().find(|s| s.id() == id) {
                return Some(sub);
            }
        }
    }
    None
}

pub fn find_group(config: &Config, group_id: ItemId) -> Option<&GroupItem> {
    config.items.iter().find_map(|i| match i {
        LaunchItem::Group(g) if g.id == group_id => Some(g),
        _ => None,
    })
}

fn find_group_mut(config: &mut Config, group_id: ItemId) -> Option<&mut GroupItem> {
    config.items.iter_mut().find_map(|i| match i {
        LaunchItem::Group(g) if g.id == group_id => Some(g),
        _ => None,
    })
}

/// All groups at the root, in display order.
pub fn groups(config: &Config) -> Vec<(ItemId, String)> {
    config
        .items
        .iter()
        .filter_map(|i| match i {
            LaunchItem::Group(g) => Some((g.id, g.title.clone())),
            _ => None,
        })
        .collect()
}

// ─── Adding ──────────────────────────────────────────────

fn insert(config: &mut Config, parent: Parent, item: LaunchItem) -> Result<ItemId, String> {
    if parent.is_some() && matches!(item, LaunchItem::Group(_)) {
        return Err("Groups can't be nested".into());
    }
    let id = item.id();
    log::info!("Added {} (id: {id})", item.title());
    list_mut(config, parent).ok_or("Group not found")?.push(item);
    Ok(id)
}

pub fn add_app(config: &mut Config, parent: Parent, title: String, path: PathBuf) -> Result<ItemId, String> {
    insert(config, parent, LaunchItem::App(AppItem::new(title, path)))
}

pub fn add_folder(config: &mut Config, parent: Parent, title: String, path: PathBuf) -> Result<ItemId, String> {
    insert(config, parent, LaunchItem::Folder(FolderItem::new(title, path)))
}

pub fn add_group(config: &mut Config, title: String) -> ItemId {
    let g = GroupItem::new(title);
    let id = g.id;
    config.items.push(LaunchItem::Group(g));
    id
}

/// Add a dropped / picked path: directories become folder shortcuts, anything else an
/// app. Returns `None` for paths that can't be launched.
pub fn add_path(config: &mut Config, parent: Parent, path: PathBuf) -> Option<ItemId> {
    if path.is_dir() {
        let title =
            path.file_name().and_then(|n| n.to_str()).map(str::to_owned).unwrap_or_else(|| path.display().to_string());
        return add_folder(config, parent, title, path).ok();
    }
    if !path.is_file() {
        return None;
    }
    let title = path.file_stem().and_then(|n| n.to_str()).unwrap_or("App").to_string();
    add_app(config, parent, title, path).ok()
}

// ─── Editing ─────────────────────────────────────────────

pub fn rename_item(config: &mut Config, id: ItemId, new_title: String) -> Result<(), String> {
    let title = new_title.trim();
    if title.is_empty() {
        return Err("Name can't be empty".into());
    }
    let item = find_mut(config, id).ok_or("Item not found")?;
    match item {
        LaunchItem::App(a) => a.title = title.to_owned(),
        LaunchItem::Group(g) => g.title = title.to_owned(),
        LaunchItem::Folder(f) => f.title = title.to_owned(),
    }
    Ok(())
}

pub fn set_icon(config: &mut Config, id: ItemId, icon_path: Option<PathBuf>) -> Result<(), String> {
    let item = find_mut(config, id).ok_or("Item not found")?;
    match item {
        LaunchItem::App(a) => a.icon_path = icon_path,
        LaunchItem::Group(g) => g.icon_path = icon_path,
        LaunchItem::Folder(f) => f.icon_path = icon_path,
    }
    Ok(())
}

pub fn icon_path(item: &LaunchItem) -> Option<&PathBuf> {
    match item {
        LaunchItem::App(a) => a.icon_path.as_ref(),
        LaunchItem::Group(g) => g.icon_path.as_ref(),
        LaunchItem::Folder(f) => f.icon_path.as_ref(),
    }
}

// ─── Removing ────────────────────────────────────────────

/// Remove an item from wherever it is (a group is removed with its contents).
pub fn remove_item(config: &mut Config, id: ItemId) -> Result<LaunchItem, String> {
    let (_, parent) = find(config, id).ok_or("Item not found")?;
    let items = list_mut(config, parent).ok_or("Item not found")?;
    let pos = items.iter().position(|i| i.id() == id).ok_or("Item not found")?;
    let removed = items.remove(pos);
    log::info!("Removed {} (id: {id})", removed.title());
    Ok(removed)
}

/// Delete a group but keep its items: they move to the root where the group was.
pub fn ungroup(config: &mut Config, group_id: ItemId) -> Result<(), String> {
    let pos = config
        .items
        .iter()
        .position(|i| matches!(i, LaunchItem::Group(g) if g.id == group_id))
        .ok_or("Group not found")?;
    let LaunchItem::Group(g) = config.items.remove(pos) else { unreachable!() };
    for (k, item) in g.items.into_iter().enumerate() {
        config.items.insert(pos + k, item);
    }
    Ok(())
}

// ─── Moving ──────────────────────────────────────────────

/// Move an app or folder into a group (from the root or from another group). The item
/// keeps its id and custom icon.
pub fn move_to_group(config: &mut Config, item_id: ItemId, group_id: ItemId) -> Result<(), String> {
    let (item, parent) = find(config, item_id).ok_or("Item not found")?;
    if matches!(item, LaunchItem::Group(_)) {
        return Err("Groups can't be nested".into());
    }
    if parent == Some(group_id) {
        return Ok(());
    }
    find_group(config, group_id).ok_or("Group not found")?;
    let item = remove_item(config, item_id)?;
    find_group_mut(config, group_id).ok_or("Group not found")?.items.push(item);
    log::info!("Moved {item_id} to group {group_id}");
    Ok(())
}

/// Move an item out of its group to the root, right after the group.
pub fn move_to_root(config: &mut Config, item_id: ItemId) -> Result<(), String> {
    let (_, parent) = find(config, item_id).ok_or("Item not found")?;
    let Some(gid) = parent else { return Ok(()) };
    let item = remove_item(config, item_id)?;
    let after = config.items.iter().position(|i| i.id() == gid).map_or(config.items.len(), |p| p + 1);
    config.items.insert(after, item);
    Ok(())
}

/// Reorder within `parent`: move the item at `from` so it ends up at index `to`.
pub fn reorder(config: &mut Config, parent: Parent, from: usize, to: usize) -> Result<(), String> {
    let items = list_mut(config, parent).ok_or("Group not found")?;
    if from >= items.len() {
        return Err("Index out of range".into());
    }
    let to = to.min(items.len() - 1);
    if from != to {
        let item = items.remove(from);
        items.insert(to, item);
    }
    Ok(())
}

// ─── Search ──────────────────────────────────────────────

/// Case-insensitive title search across the root and all groups. Groups themselves
/// match too; results keep display order.
pub fn search(config: &Config, query: &str) -> Vec<(LaunchItem, Parent)> {
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    for item in &config.items {
        if item.title().to_lowercase().contains(&q) {
            out.push((item.clone(), None));
        }
        if let LaunchItem::Group(g) = item {
            for sub in &g.items {
                if sub.title().to_lowercase().contains(&q) {
                    out.push((sub.clone(), Some(g.id)));
                }
            }
        }
    }
    out
}

/// "New Group", "New Group 2", ... — the first name not already taken.
pub fn unique_group_name(config: &Config, base: &str) -> String {
    let taken: Vec<String> = groups(config).into_iter().map(|(_, n)| n).collect();
    if !taken.iter().any(|n| n == base) {
        return base.to_owned();
    }
    (2..).map(|i| format!("{base} {i}")).find(|c| !taken.contains(c)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    fn titles(items: &[LaunchItem]) -> Vec<&str> {
        items.iter().map(|i| i.title()).collect()
    }

    #[test]
    fn add_and_remove_app() {
        let mut c = cfg();
        let id = add_app(&mut c, None, "Firefox".into(), "C:/firefox.exe".into()).unwrap();
        assert_eq!(titles(&c.items), ["Firefox"]);
        remove_item(&mut c, id).unwrap();
        assert!(c.items.is_empty());
    }

    #[test]
    fn move_app_and_folder_into_group_keeps_identity() {
        let mut c = cfg();
        let app = add_app(&mut c, None, "Steam".into(), "C:/steam.exe".into()).unwrap();
        let folder = add_folder(&mut c, None, "Saves".into(), "C:/saves".into()).unwrap();
        set_icon(&mut c, app, Some("custom.png".into())).unwrap();
        let g = add_group(&mut c, "Games".into());

        move_to_group(&mut c, app, g).unwrap();
        move_to_group(&mut c, folder, g).unwrap();

        assert_eq!(titles(&c.items), ["Games"]);
        let group = find_group(&c, g).unwrap();
        assert_eq!(titles(&group.items), ["Steam", "Saves"]);
        let (moved, parent) = find(&c, app).unwrap();
        assert_eq!(parent, Some(g));
        assert_eq!(icon_path(moved), Some(&PathBuf::from("custom.png")));
    }

    #[test]
    fn move_between_groups_and_back_to_root() {
        let mut c = cfg();
        let a = add_group(&mut c, "A".into());
        let b = add_group(&mut c, "B".into());
        let app = add_app(&mut c, Some(a), "Tool".into(), "C:/tool.exe".into()).unwrap();

        move_to_group(&mut c, app, b).unwrap();
        assert!(find_group(&c, a).unwrap().items.is_empty());
        assert_eq!(find(&c, app).unwrap().1, Some(b));

        move_to_root(&mut c, app).unwrap();
        assert_eq!(titles(&c.items), ["A", "B", "Tool"], "lands right after its old group");
        assert_eq!(find(&c, app).unwrap().1, None);
    }

    #[test]
    fn groups_cannot_nest() {
        let mut c = cfg();
        let g1 = add_group(&mut c, "Group 1".into());
        let g2 = add_group(&mut c, "Group 2".into());
        assert!(move_to_group(&mut c, g1, g2).is_err());
    }

    #[test]
    fn ungroup_keeps_items_in_place() {
        let mut c = cfg();
        add_app(&mut c, None, "First".into(), "C:/1.exe".into()).unwrap();
        let g = add_group(&mut c, "G".into());
        add_app(&mut c, None, "Last".into(), "C:/3.exe".into()).unwrap();
        add_app(&mut c, Some(g), "X".into(), "C:/x.exe".into()).unwrap();
        add_app(&mut c, Some(g), "Y".into(), "C:/y.exe".into()).unwrap();

        ungroup(&mut c, g).unwrap();
        assert_eq!(titles(&c.items), ["First", "X", "Y", "Last"]);
    }

    #[test]
    fn reorder_moves_item() {
        let mut c = cfg();
        for t in ["A", "B", "C", "D"] {
            add_app(&mut c, None, t.into(), format!("C:/{t}.exe").into()).unwrap();
        }
        reorder(&mut c, None, 0, 2).unwrap();
        assert_eq!(titles(&c.items), ["B", "C", "A", "D"]);
        reorder(&mut c, None, 3, 0).unwrap();
        assert_eq!(titles(&c.items), ["D", "B", "C", "A"]);
        reorder(&mut c, None, 1, 99).unwrap();
        assert_eq!(titles(&c.items), ["D", "C", "A", "B"]);
    }

    #[test]
    fn rename_any_item() {
        let mut c = cfg();
        let id = add_app(&mut c, None, "Chrome".into(), "C:/chrome.exe".into()).unwrap();
        rename_item(&mut c, id, "  Google Chrome ".into()).unwrap();
        assert_eq!(c.items[0].title(), "Google Chrome");
        assert!(rename_item(&mut c, id, "   ".into()).is_err());
    }

    #[test]
    fn search_spans_groups() {
        let mut c = cfg();
        add_app(&mut c, None, "Notepad".into(), "C:/n.exe".into()).unwrap();
        let g = add_group(&mut c, "Tools".into());
        add_app(&mut c, Some(g), "Notepad++".into(), "C:/npp.exe".into()).unwrap();
        let hits = search(&c, "NOTE");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[1].1, Some(g));
    }

    #[test]
    fn unique_names() {
        let mut c = cfg();
        assert_eq!(unique_group_name(&c, "New Group"), "New Group");
        add_group(&mut c, "New Group".into());
        assert_eq!(unique_group_name(&c, "New Group"), "New Group 2");
    }

    #[test]
    fn remove_nonexistent() {
        let mut c = cfg();
        assert!(remove_item(&mut c, ItemId::new()).is_err());
    }

    #[test]
    fn config_json_roundtrip() {
        let mut c = cfg();
        add_app(&mut c, None, "Notepad".into(), "C:/Windows/notepad.exe".into()).unwrap();
        add_group(&mut c, "Utilities".into());
        let json = serde_json::to_string_pretty(&c).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(titles(&parsed.items), ["Notepad", "Utilities"]);
    }
}
