mod local;
mod network;
mod protected;
mod recursive;
pub mod tracked;

pub use recursive::delete_recursive;

use crate::domain::model::{AppMsg, ArchiveMsg, FluxApp, WindowMsg};
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

/// Deletes (trashes, or recursively removes) the given selection.
///
/// If `selection` is empty, falls back to `active_item_path`. Protected paths
/// are rejected with a toast and skipped. Archive virtual URIs are routed to
/// the archive deletion prompt instead of being touched on disk.
pub fn delete_items(
    selection: Vec<PathBuf>,
    active_item_path: Option<PathBuf>,
    sender: AsyncComponentSender<FluxApp>,
) {
    crate::hit!("delete_items");
    let mut selection = selection;
    if selection.is_empty() {
        if let Some(active) = active_item_path {
            selection.push(active);
        }
    }

    if selection.is_empty() {
        return;
    }

    let sender_clone = sender.clone();
    let mut local_batch: Vec<(gio::File, PathBuf)> = Vec::new();
    for raw_path in selection {
        // Resolve only for protection checks, use raw_path for deletion. A symlink
        // in the last component is not followed: trashing it removes the link.
        let canon_path = protected::resolve_entry(&raw_path);

        if protected::is_protected_target(&canon_path) {
            eprintln!(
                "[Delete] Blocked attempt to delete protected system path: {:?}",
                canon_path
            );
            sender_clone.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                "Cannot delete protected path: {}",
                canon_path.display()
            ))));
            continue;
        }

        let path_str = raw_path.to_string_lossy().into_owned();

        if path_str.starts_with(crate::infra::archive::ARCHIVE_URI)
            || path_str.starts_with("/archive:/")
            || path_str.starts_with("archive://")
        {
            if let Some((archive_path, inner_path)) =
                crate::infra::archive::parse_archive_uri(&path_str)
            {
                sender_clone.input(AppMsg::Archive(ArchiveMsg::PromptArchiveDeletion {
                    archive_path,
                    inner_path,
                }));
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

        if is_network {
            network::delete_via_recursive(file, sender_clone.clone());
        } else {
            local_batch.push((file, raw_path));
        }
    }

    if !local_batch.is_empty() {
        local::delete_batch_via_trash(local_batch, sender_clone);
    }
}
