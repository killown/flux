use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::ui::undo_redo::FileOp;
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Deletes all files currently residing in the virtual system trash directory.
    pub fn handle_empty_trash(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_empty_trash");
        let root = gio::File::for_uri(constants::TRASH_URI);
        if let Ok(enumerator) = root.enumerate_children(
            "standard::name",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        ) {
            for info in enumerator.flatten() {
                let _ = root.child(info.name()).delete(gio::Cancellable::NONE);
            }
        }
        sender.input(AppMsg::Refresh);
    }

    /// Records a successful trash batch in the undo history and evicts any
    /// affected folder-cache entries.
    ///
    /// Called for both direct deletes and undo/redo-driven trash operations.
    /// Only use this when the operation moved items to the system trash, for
    /// recursive deletes that bypass trash, call [`Self::record_delete`]
    /// instead.
    pub fn record_trash(&mut self, paths: Vec<PathBuf>, sender: &AsyncComponentSender<Self>) {
        self.evict_caches_for_paths(&paths);
        self.file_op_history.push_undo(FileOp::Trash { paths });
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
    /// `AppMsg::FileDeleted` and remove rows organically. Tag views have no
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
