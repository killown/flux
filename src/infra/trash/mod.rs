//! Deleting files: classifying a selection and the blocking filesystem work.

mod local;
mod network;
mod protected;
mod recursive;
pub mod tracked;

pub use local::trash_local_batch;
pub use network::delete_network_uri;
pub use recursive::delete_recursive;

use gtk::gio;
use gtk::gio::prelude::FileExt;
use std::path::PathBuf;

/// URI of the virtual trash folder.
pub const TRASH_URI: &str = "trash:///";

/// A selection split by how each entry has to be deleted.
#[derive(Debug, Default)]
pub struct DeletePlan {
    /// Resolved paths that were rejected because deleting them is never allowed.
    pub protected: Vec<PathBuf>,
    /// Entries inside an archive, as `(archive_path, inner_path)`. They are
    /// never touched on disk and need a confirmation prompt instead.
    pub archives: Vec<(PathBuf, String)>,
    /// URIs on network filesystems, which have no trash and are deleted recursively.
    pub network: Vec<String>,
    /// Local entries as `(uri, original_path)`, moved to the trash as one batch.
    /// URIs are used because `gio::File` is not `Send`.
    pub local: Vec<(String, PathBuf)>,
}

/// Splits `selection` into a [`DeletePlan`].
///
/// If `selection` is empty, falls back to `active_item_path`. Protected paths
/// and archive virtual URIs are separated out instead of being scheduled for
/// deletion. Does no deletion itself.
pub fn plan_deletion(selection: Vec<PathBuf>, active_item_path: Option<PathBuf>) -> DeletePlan {
    crate::hit!("plan_deletion");
    let mut selection = selection;
    if selection.is_empty() {
        if let Some(active) = active_item_path {
            selection.push(active);
        }
    }

    let mut plan = DeletePlan::default();
    for raw_path in selection {
        // Resolve only for protection checks, use raw_path for deletion. A symlink
        // in the last component is not followed: trashing it removes the link.
        let canon_path = protected::resolve_entry(&raw_path);

        if protected::is_protected_target(&canon_path) {
            eprintln!(
                "[Delete] Blocked attempt to delete protected system path: {:?}",
                canon_path
            );
            plan.protected.push(canon_path);
            continue;
        }

        let path_str = raw_path.to_string_lossy().into_owned();

        if path_str.starts_with(crate::infra::archive::ARCHIVE_URI)
            || path_str.starts_with("/archive:/")
            || path_str.starts_with("archive://")
        {
            if let Some(target) = crate::infra::archive::parse_archive_uri(&path_str) {
                plan.archives.push(target);
            }
            continue;
        }

        let is_network = crate::infra::network::is_network_uri(&raw_path);

        eprintln!(
            "[Delete] path={:?} is_network={} contains_scheme={}",
            path_str,
            is_network,
            path_str.contains("://")
        );

        let file = if path_str.contains("://") {
            gio::File::for_uri(&path_str)
        } else {
            gio::File::for_path(&raw_path)
        };
        let uri = file.uri().to_string();

        if is_network {
            plan.network.push(uri);
        } else {
            plan.local.push((uri, raw_path));
        }
    }
    plan
}
