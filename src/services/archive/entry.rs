//! Archive entry model and conversion to grid load contexts.

use crate::model::FileLoadContext;
use crate::services::archive::build_archive_uri;
use std::collections::HashMap;
use std::path::Path;

/// Describes a single entry within an archive as seen from a specific directory level.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub is_dir: bool,
    /// Uncompressed size in bytes, `0` for synthesised directory nodes.
    pub size: u64,
    /// Last-modified unix timestamp, `0` when unavailable.
    pub mtime: i64,
    /// Full inner path relative to the archive root, used to build child URIs.
    pub inner_path: String,
    /// Number of direct children inside this directory at the listed level.
    /// Always `0` for file entries. Populated by `collect_entry`.
    pub child_count: u64,
    #[allow(dead_code)]
    pub is_encrypted: bool,
}

/// Converts [`ArchiveEntry`] items into [`FileLoadContext`] records for the grid model.
#[allow(dead_code)]
pub fn entries_to_load_contexts(
    entries: &[ArchiveEntry],
    archive_path: &Path,
    expand_labels: bool,
) -> Vec<FileLoadContext> {
    crate::hit!("entries_to_load_contexts");
    entries
        .iter()
        .map(|e| {
            let target_path = build_archive_uri(archive_path, &e.inner_path);

            // For directories, count the immediate children visible in this listing
            // (i.e. the other entries that were returned alongside this one).
            // `e.size` is always 0 for synthesised archive directory nodes, so we
            // derive the count from the sibling entries instead.
            let size = if e.is_dir { e.child_count } else { e.size };

            let sort_ext = std::path::Path::new(&e.name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();

            let custom_icon = if !e.is_dir && !sort_ext.is_empty() {
                crate::services::loader::get_extension_icon_path(&sort_ext)
                    .map(|p| p.to_string_lossy().into_owned())
            } else {
                None
            };

            FileLoadContext::with_stats(
                e.name.clone(),
                target_path,
                e.is_dir,
                e.name.to_lowercase(),
                sort_ext,
                size,
                e.mtime,
                None,
                expand_labels,
                custom_icon,
            )
        })
        .collect()
}

/// Resolves one archive entry path against `prefix` and inserts the immediate
/// child (real or synthesised directory node) into `seen`.
pub(super) fn collect_entry(
    seen: &mut HashMap<String, ArchiveEntry>,
    raw_name: &str,
    is_entry_dir: bool,
    size: u64,
    mtime: i64,
    prefix: &str,
    is_encrypted: bool,
) {
    let relative = if prefix.is_empty() {
        raw_name.to_owned()
    } else {
        let pfx = prefix.trim_end_matches('/');
        match raw_name.strip_prefix(&format!("{pfx}/")) {
            Some(r) if !r.is_empty() => r.to_owned(),
            _ => return,
        }
    };

    let (child_name, is_dir, child_inner, contributes_child) = match relative.find('/') {
        Some(slash_pos) => {
            let dir_name = &relative[..slash_pos];
            let inner = if prefix.is_empty() {
                dir_name.to_owned()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), dir_name)
            };
            // This raw entry contributes a child to the synthesised dir node
            (dir_name.to_owned(), true, inner, true)
        }
        None => {
            let inner = if prefix.is_empty() {
                relative.clone()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), relative)
            };
            // Leaf entry at this level, contributes itself, not a child to a dir
            (relative, is_entry_dir, inner, false)
        }
    };

    let entry_size = if is_dir { 0 } else { size };

    let _entry = seen
        .entry(child_name.clone())
        .and_modify(|existing| {
            if !is_dir && !is_entry_dir {
                existing.is_dir = false;
                existing.size = size;
                existing.mtime = mtime;
            }
            // Every raw entry that maps to this dir node is a direct child
            if contributes_child {
                existing.child_count += 1;
            }
        })
        .or_insert(ArchiveEntry {
            name: child_name,
            is_dir,
            size: entry_size,
            mtime,
            inner_path: child_inner,
            child_count: if contributes_child { 1 } else { 0 },
            is_encrypted,
        });
}
