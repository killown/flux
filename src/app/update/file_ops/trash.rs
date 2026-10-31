use crate::app::state::FluxApp;
use crate::domain::model::{
    AppMsg, ArchiveMsg, FileOp, FileOpsMsg, NavigationMsg, TrashedItem, WindowMsg,
};
use crate::infra::trash::{delete_network_uri, plan_deletion, trash_local_batch, TRASH_URI};
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Deletes (trashes, or recursively removes) the given selection.
    pub fn handle_delete_items(
        &self,
        selection: Vec<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_delete_items");
        let plan = plan_deletion(selection, self.active_item_path.clone());

        for path in plan.protected {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                "Cannot delete protected path: {}",
                path.display()
            ))));
        }

        for (archive_path, inner_path) in plan.archives {
            sender.input(AppMsg::Archive(ArchiveMsg::PromptArchiveDeletion {
                archive_path,
                inner_path,
            }));
        }

        for uri in plan.network {
            let sender = sender.clone();
            relm4::spawn_blocking(move || match delete_network_uri(&uri) {
                Ok(()) => sender.input(AppMsg::Navigation(NavigationMsg::Refresh)),
                Err(e) => {
                    let toast = if e.message().contains("Permission denied")
                        || e.message().contains("Operation not permitted")
                    {
                        "Permission denied: Cannot delete item.".to_string()
                    } else {
                        format!("Deletion error: {e}")
                    };
                    sender.input(AppMsg::Window(WindowMsg::ShowToast(toast)));
                }
            });
        }

        if !plan.local.is_empty() {
            let sender = sender.clone();
            let targets = plan.local;
            relm4::spawn_blocking(move || {
                let (trashed, failed) = trash_local_batch(targets);

                for failure in failed {
                    sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "{}: {}",
                        crate::i18n::tr("Could not move to trash"),
                        failure.error.message()
                    ))));
                }

                if !trashed.is_empty() {
                    sender.input(AppMsg::FileOps(FileOpsMsg::TrashSucceeded(trashed)));
                    sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
                }
            });
        }
    }

    /// Deletes all files currently residing in the virtual system trash directory.
    ///
    /// The deletion runs on a worker thread. Folders are removed recursively,
    /// and a toast reports any entries that could not be deleted.
    pub fn handle_empty_trash(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_empty_trash");
        let sender = sender.clone();
        relm4::spawn_blocking(move || {
            let root = gio::File::for_uri(TRASH_URI);
            let mut failed = 0usize;
            if let Ok(enumerator) = root.enumerate_children(
                "standard::name",
                gio::FileQueryInfoFlags::NONE,
                gio::Cancellable::NONE,
            ) {
                for info in enumerator.flatten() {
                    let child = root.child(info.name());
                    if crate::infra::trash::delete_recursive(&child).is_err() {
                        failed += 1;
                    }
                }
            }
            if failed > 0 {
                sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                    "{}: {}",
                    crate::i18n::tr("Could not delete some items from the trash"),
                    failed
                ))));
            }
            sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
        });
    }

    /// Records a successful trash batch in the undo history and evicts any
    /// affected folder-cache entries.
    ///
    /// Called for both direct deletes and undo/redo-driven trash operations.
    /// Only use this when the operation moved items to the system trash, for
    /// recursive deletes that bypass trash, call [`Self::record_delete`]
    /// instead.
    pub fn record_trash(&mut self, items: Vec<TrashedItem>, sender: &AsyncComponentSender<Self>) {
        let paths: Vec<PathBuf> = items.iter().map(|i| i.path.clone()).collect();
        self.evict_caches_for_paths(&paths);
        self.file_op_history.push_undo(FileOp::Trash { items });
        let _ = sender;
    }

    /// Drops every folder-cache entry whose key is a parent of one of `paths`
    /// and the cache key for the current directory.
    fn evict_caches_for_paths(&mut self, paths: &[PathBuf]) {
        crate::hit!("evict_caches_for_paths");
        let mut parents: Vec<PathBuf> = Vec::with_capacity(paths.len() + 1);
        for p in paths {
            if let Some(parent) = p.parent() {
                parents.push(self.cache_key(parent));
            }
        }
        parents.push(self.cache_key(&self.current_path));

        for key in parents {
            self.folder_cache.remove(&key);
        }
    }

    /// Removes every grid entry pointing at one of `paths` across all tabs,
    /// regardless of whether the current view is a directory, an archive, or
    /// a tag-filtered result set.
    ///
    /// Directory views normally rely on the GIO directory monitor to dispatch
    /// `FileOpsMsg::FileDeleted` and remove rows organically. Tag views have no
    /// monitor, so deleted files would otherwise linger as orphaned
    /// [`FileItem`]s pointing at nonexistent paths, and `Refresh` deliberately
    /// no-ops for `#tag` filters (see [`Self::handle_refresh_path`]).
    pub fn purge_paths_from_all_tabs(&mut self, paths: &[PathBuf]) {
        crate::hit!("purge_paths_from_all_tabs");
        for tab in &mut self.tabs {
            let mut i = 0;
            while i < tab.files.len() {
                let removed = tab
                    .files
                    .get(i)
                    .is_some_and(|w| paths.contains(&w.borrow().path));
                if removed {
                    tab.files.remove(i);
                } else {
                    i += 1;
                }
            }
        }
    }
}
