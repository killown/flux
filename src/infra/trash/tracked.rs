//! Trash operations that remember *which* trash entry they created.
//!
//! `gio::File::trash` does not say what the new entry in `trash:///` is
//! called. Several entries can share one original path (delete a file,
//! recreate it, delete it again), so undo must restore the specific entry a
//! deletion produced instead of every entry with a matching `orig-path`.

use crate::ui::constants::TRASH_URI;
use crate::ui::undo_redo::TrashedItem;
use gtk::gio::prelude::*;
use gtk::{gio, glib};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const TRASH_ATTRS: &str = "standard::name,trash::orig-path,trash::deletion-date";

/// One entry currently present in `trash:///`.
#[derive(Debug, Clone)]
pub struct TrashEntry {
    pub name: String,
    pub orig_path: Option<String>,
    pub deletion_date: Option<String>,
}

fn string_attr(info: &gio::FileInfo, attr: &str) -> Option<String> {
    info.attribute_as_string(attr)
        .map(|s| s.to_string())
        .or_else(|| {
            info.attribute_byte_string(attr)
                .map(|bytes| String::from_utf8_lossy(bytes.as_bytes()).to_string())
        })
}

/// Snapshot of everything currently in the trash. Blocking: call it from a
/// worker thread.
pub fn list_trash() -> Vec<TrashEntry> {
    let root = gio::File::for_uri(TRASH_URI);
    let Ok(enumerator) = root.enumerate_children(
        TRASH_ATTRS,
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    ) else {
        return Vec::new();
    };
    enumerator
        .flatten()
        .map(|info| TrashEntry {
            name: info.name().to_string_lossy().into_owned(),
            orig_path: string_attr(&info, "trash::orig-path"),
            deletion_date: string_attr(&info, "trash::deletion-date"),
        })
        .collect()
}

/// Picks the entry created by a trash call, given the trash contents before
/// (`before`) and after (`after`) it.
///
/// Only entries that were not there before are candidates. Among those, an
/// entry whose original path matches `orig_path` wins (newest deletion date
/// first). If none match by path (for example the trash records a resolved
/// symlink path), a single new entry is still unambiguous and is used.
pub fn pick_new_entry(
    before: &HashSet<String>,
    after: Vec<TrashEntry>,
    orig_path: &str,
) -> Option<TrashEntry> {
    let new: Vec<TrashEntry> = after
        .into_iter()
        .filter(|e| !before.contains(&e.name))
        .collect();
    let matching = new
        .iter()
        .filter(|e| e.orig_path.as_deref() == Some(orig_path))
        .max_by(|a, b| a.deletion_date.cmp(&b.deletion_date))
        .cloned();
    match matching {
        Some(entry) => Some(entry),
        None if new.len() == 1 => new.into_iter().next(),
        None => None,
    }
}

/// Moves `file` to the trash and returns a record of the entry it created.
///
/// Blocking: call it from a worker thread. `orig_path` is the path the item
/// is known by in the app and is stored in the returned item.
pub fn trash_tracked(file: &gio::File, orig_path: &Path) -> Result<TrashedItem, glib::Error> {
    let orig_str = file
        .path()
        .unwrap_or_else(|| orig_path.to_path_buf())
        .to_string_lossy()
        .into_owned();

    let before: HashSet<String> = list_trash().into_iter().map(|e| e.name).collect();
    file.trash(gio::Cancellable::NONE)?;
    let entry = pick_new_entry(&before, list_trash(), &orig_str);

    Ok(TrashedItem {
        path: orig_path.to_path_buf(),
        trash_name: entry.as_ref().map(|e| e.name.clone()),
        deletion_date: entry.and_then(|e| e.deletion_date),
    })
}

/// Finds the trash entry that belongs to `item`, ignoring entries in `used`.
///
/// With a tracked name the entry must have that name, and if both sides know
/// a deletion date the dates must agree (a name can be reused after the
/// trash was emptied). Without a tracked name, the newest entry with a
/// matching original path is used.
pub fn find_entry<'a>(
    entries: &'a [TrashEntry],
    item: &TrashedItem,
    used: &HashSet<String>,
) -> Option<&'a TrashEntry> {
    if let Some(name) = &item.trash_name {
        return entries.iter().find(|e| {
            &e.name == name
                && !used.contains(&e.name)
                && match (&item.deletion_date, &e.deletion_date) {
                    (Some(want), Some(have)) => want == have,
                    _ => true,
                }
        });
    }
    let orig = item.path.to_string_lossy();
    entries
        .iter()
        .filter(|e| e.orig_path.as_deref() == Some(&*orig) && !used.contains(&e.name))
        .max_by(|a, b| a.deletion_date.cmp(&b.deletion_date))
}

/// Restores each item from the trash to its original location.
///
/// Returns `(restored, failed)`. Each item is handled on its own, so one
/// failure does not undo or hide the items that were restored. Blocking:
/// call it from a worker thread.
pub fn restore_tracked(items: Vec<TrashedItem>) -> (Vec<TrashedItem>, Vec<TrashedItem>) {
    let entries = list_trash();
    let root = gio::File::for_uri(TRASH_URI);
    let mut used: HashSet<String> = HashSet::new();
    let mut restored = Vec::new();
    let mut failed = Vec::new();

    for item in items {
        let Some(entry) = find_entry(&entries, &item, &used) else {
            eprintln!("[Restore] no matching trash entry for {:?}", item.path);
            failed.push(item);
            continue;
        };

        let dest_path: PathBuf = entry
            .orig_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| item.path.clone());
        let trash_item = root.child(&entry.name);
        let dest = gio::File::for_path(&dest_path);
        let result = trash_item.move_(
            &dest,
            gio::FileCopyFlags::NONE,
            gio::Cancellable::NONE,
            None,
        );

        match result {
            Ok(()) => {
                used.insert(entry.name.clone());
                restored.push(item);
            }
            Err(e) => {
                eprintln!("[Restore] move_ failed for {:?}: {}", dest_path, e);
                failed.push(item);
            }
        }
    }

    (restored, failed)
}
