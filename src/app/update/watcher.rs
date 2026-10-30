use crate::domain::model::{AppMsg, FluxApp};
use crate::ui::FileItem;
use crate::utils;
use adw::gio::prelude::*;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

thread_local! {
    /// Pending debounce timers for watcher-driven `Changed` events, keyed on
    /// path. Each entry is the currently scheduled dispatch for that path,
    /// a new event for the same path replaces the entry, so only the last
    /// event in a burst survives.
    static PENDING_CHANGED: std::cell::RefCell<
        std::collections::HashMap<PathBuf, glib::SourceId>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Suffixes used by download tools while a file is still being written.
///
/// Firefox: `<name>.<random>.RW2.part`
/// Chromium: `<name>.crdownload`
/// wget: `<name>.tmp`
///
/// Watcher events for paths with these suffixes are ignored entirely,
/// they are internal scratch files, never visible to the user, and
/// thumbnailing them wastes cycles on partial data.
const DOWNLOAD_TEMP_SUFFIXES: &[&str] = &["part", "crdownload", "download", "partial", "tmp"];

/// Coalesces rapid-fire `Changed` events for the same path into a single
/// dispatch after the writer stops.
///
/// A 30 MB download emits dozens of `Changed` events, one per write flush.
/// Forwarding each one to `handle_file_changed` races the thumbnail pipeline
/// against a truncated file, which produces errors like "Unidentified RW2
/// file" from exiftool. This debounces the dispatch so the handler only runs
/// once the writer has stopped.
pub(crate) fn debounce_path_event(path: PathBuf, sender: relm4::AsyncComponentSender<FluxApp>) {
    crate::hit!("debounce_path_event");
    PENDING_CHANGED.with(|map| {
        let mut map = map.borrow_mut();

        if let Some(old) = map.remove(&path) {
            old.remove();
        }

        let path_for_timeout = path.clone();
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
            PENDING_CHANGED.with(|map| {
                map.borrow_mut().remove(&path_for_timeout);
            });
            sender.input(AppMsg::FileChanged(path_for_timeout));
        });
        map.insert(path, id);
    });
}

/// Returns `true` when `path` is a download scratch file that the user
/// should never see.
fn is_download_temp(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        let lower = e.to_ascii_lowercase();
        DOWNLOAD_TEMP_SUFFIXES.contains(&lower.as_str())
    })
}

/// Returns `true` when a sibling `.part` (or `.crdownload` etc.) file exists
/// in the same directory that appears to belong to `path`.
///
/// Download tools create the final path as an empty placeholder, then write
/// to `<name>.<random>.<ext>.part` next to it, and rename on completion. As
/// long as that sibling exists, the download is still in progress and the
/// final file is not safe to thumbnail.
///
/// Matching is done on the file stem: `DSC_4207.NEF` matches
/// `DSC_4207.L8Koxwkl.NEF.part` because both start with `DSC_4207`.
///
/// This reads the entire parent directory with blocking I/O, so it must not
/// run on the GTK thread. The watcher reaches it through
/// `spawn_thumbnail_loader_unless_downloading`, which runs it on the blocking pool.
pub(crate) fn has_active_download_sibling(path: &Path) -> bool {
    crate::hit!("has_active_download_sibling");
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    if stem.is_empty() {
        return false;
    }

    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only consider siblings that look like download scratch files.
        let is_temp = Path::new(name_str.as_ref())
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                let lower = e.to_ascii_lowercase();
                DOWNLOAD_TEMP_SUFFIXES.contains(&lower.as_str())
            });
        if !is_temp {
            continue;
        }

        // The scratch file must share the target's stem prefix. This avoids
        // false positives from unrelated downloads in the same directory.
        if name_str.starts_with(stem) {
            return true;
        }
    }

    false
}

impl FluxApp {
    pub fn handle_file_deleted(&mut self, path: PathBuf) {
        crate::hit!("FluxApp::handle_file_deleted");
        crate::infra::search::indexer::notify_deleted(&path);

        self.evict_folder_cache(&path);

        if self.is_content_searching {
            // A file can have multiple result rows (one per matching line),
            // remove all of them
            let mut i = 0;
            while i < self.tabs[self.active_tab_index].files.len() {
                if self.tabs[self.active_tab_index]
                    .files
                    .get(i)
                    .is_some_and(|r| r.borrow().path == path)
                {
                    self.tabs[self.active_tab_index].files.remove(i);
                    // don't increment i, next item shifts into this slot
                } else {
                    i += 1;
                }
            }
            return;
        }

        // Try exact path match first, then fall back to filename match.
        // The monitor path may be canonicalized (symlinks resolved) while
        // grid items store current_path.join(name), so they can differ.
        let target_idx = (0..self.tabs[self.active_tab_index].files.len())
            .find(|&i| {
                self.tabs[self.active_tab_index]
                    .files
                    .get(i)
                    .is_some_and(|r| r.borrow().path == path)
            })
            .or_else(|| {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .and_then(|name| {
                        (0..self.tabs[self.active_tab_index].files.len()).find(|&i| {
                            self.tabs[self.active_tab_index]
                                .files
                                .get(i)
                                .is_some_and(|r| r.borrow().name == name)
                        })
                    })
            });

        if let Some(idx) = target_idx {
            self.tabs[self.active_tab_index].files.remove(idx);
        }
    }

    fn find_file_index_by_name(&self, name: &str) -> Option<u32> {
        crate::hit!("watcher::find_file_index_by_name");
        (0..self.tabs[self.active_tab_index].files.len()).find(|&i| {
            self.tabs[self.active_tab_index]
                .files
                .get(i)
                .is_some_and(|r| r.borrow().name == name)
        })
    }

    pub fn handle_file_changed(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_file_changed");

        // Ignore download scratch files entirely. They are not visible to
        // the user, and processing them wastes cycles on partial data.
        if is_download_temp(&path) {
            return;
        }

        self.evict_folder_cache(&path);

        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            return;
        };

        let file = gio::File::for_path(&path);
        let Ok(info) = file.query_info(
            "standard::name,standard::display-name,standard::type",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        ) else {
            // File vanished. Remove the row if it's there.
            crate::infra::search::indexer::notify_deleted(&path);
            if let Some(idx) = self.find_file_index_by_name(&name) {
                self.tabs[self.active_tab_index].files.remove(idx);
            }
            return;
        };

        let is_dir = info.file_type() == gio::FileType::Directory;
        let display_name = info.display_name().to_string();
        let icon = utils::icon::get_icon_for_path(&path, is_dir);

        if let Some(idx) = self.find_file_index_by_name(&name) {
            // Item exists. Only two reasons to touch it:
            //  - The icon path changed (rare, e.g. via a custom-icon reload)
            //  - It has no thumbnail yet and the file is now ready
            let (needs_icon_refresh, needs_thumbnail, grid_idx) = {
                let wrapper = self.tabs[self.active_tab_index].files.get(idx).unwrap();
                let item = wrapper.borrow();
                (
                    item.icon != icon,
                    !is_dir && item.thumbnail.is_none(),
                    item.grid_idx,
                )
            };

            if needs_icon_refresh {
                let wrapper = self.tabs[self.active_tab_index].files.get(idx).unwrap();
                let mut item = wrapper.borrow().clone();
                item.icon = icon;
                self.tabs[self.active_tab_index].files.remove(idx);
                self.tabs[self.active_tab_index].files.insert(idx, item);
            }

            if needs_thumbnail && utils::media::may_have_thumbnail(&path) {
                let current_session = self.load_id.load(Ordering::SeqCst);
                self.spawn_thumbnail_loader_unless_downloading(
                    vec![(grid_idx, path.clone())],
                    current_session,
                    self.active_tab_index,
                    sender.clone(),
                );
            }

            return;
        }

        // Item doesn't exist yet. Append it.
        crate::infra::search::indexer::notify_created(&path);

        let is_empty = if is_dir && self.config.ui.show_empty_dir_emblem {
            FluxApp::is_dir_empty(&path)
        } else {
            false
        };
        let is_symlink = path.is_symlink();
        let is_broken_symlink = is_symlink && std::fs::metadata(&path).is_err();
        let grid_idx = self.tabs[self.active_tab_index].files.len();

        self.tabs[self.active_tab_index].files.append(
            FileItem::builder(display_name, path.clone(), icon)
                .is_dir(is_dir)
                .is_empty(is_empty)
                .expand_labels(self.config.ui.expand_labels)
                .icon_size(if self.is_list_mode {
                    self.current_list_icon_size
                } else {
                    self.current_icon_size
                })
                .is_list_mode(self.is_list_mode)
                .grid_idx(grid_idx)
                .max_width_chars(self.config.ui.max_width_chars)
                .grid_spacing(self.config.ui.grid_spacing)
                .is_symlink(is_symlink)
                .is_broken_symlink(is_broken_symlink)
                .show_symlink_emblem(self.config.ui.show_symlink_emblem)
                .build(),
        );

        // Spawn a thumbnail only for file kinds we thumbnail, and only when no
        // download scratch file is present. Firefox writes
        // `DSC_4207.L8Koxwkl.NEF.part` next to `DSC_4207.NEF` for the duration
        // of the download, once the rename fires, the sibling is gone and the
        // check passes. The sibling scan itself runs off the GTK thread.
        if !is_dir && utils::media::may_have_thumbnail(&path) {
            let current_session = self.load_id.load(Ordering::SeqCst);
            self.spawn_thumbnail_loader_unless_downloading(
                vec![(grid_idx, path.clone())],
                current_session,
                self.active_tab_index,
                sender.clone(),
            );
        }

        sender.input(AppMsg::SelectionChanged { tab_id: None });
    }

    pub fn handle_start_rename(&mut self, path: PathBuf) {
        crate::hit!("handle_start_rename");
        self.active_item_path = Some(path.clone());

        let target_idx = (0..self.tabs[self.active_tab_index].files.len()).find(|&i| {
            self.tabs[self.active_tab_index]
                .files
                .get(i)
                .is_some_and(|r| r.borrow().path == path)
        });

        if let Some(idx) = target_idx {
            let item_opt = self.tabs[self.active_tab_index].files.get(idx).map(|w| {
                let mut item = w.borrow().clone();
                item.is_editing = true;
                item
            });
            if let Some(item) = item_opt {
                self.tabs[self.active_tab_index].files.remove(idx);
                self.tabs[self.active_tab_index].files.insert(idx, item);
            }
        }
    }

    pub fn handle_trigger_rename_selection(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_trigger_rename_selection");
        if let Some(path) = self.get_selected_path() {
            sender.input(AppMsg::StartRename(path));
        }
    }

    pub fn handle_file_deleted_dispatch(&mut self, path: PathBuf) {
        crate::hit!("handle_file_deleted");
        self.preview_cache.clear();
        if let Some(parent) = path.parent() {
            self.folder_cache.remove(&self.cache_key(parent));
        }
        self.handle_file_deleted(path);
    }

    pub fn handle_file_changed_dispatch(
        &mut self,
        path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_file_changed_dispatch");
        if let Some(parent) = path.parent() {
            self.folder_cache.remove(&self.cache_key(parent));
        }
        self.handle_file_changed(path, sender);
    }

    pub fn handle_item_moved(
        &mut self,
        old_path: PathBuf,
        new_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_item_moved");
        crate::infra::search::indexer::notify_moved(&old_path, &new_path);

        if let Some(p) = old_path.parent() {
            self.folder_cache.remove(&self.cache_key(p));
        }
        if let Some(p) = new_path.parent() {
            self.folder_cache.remove(&self.cache_key(p));
        }
        self.folder_cache
            .remove(&self.cache_key(&self.current_path));

        let _ = self.state_db.rename_path(&old_path, &new_path);

        let old_key = old_path.to_string_lossy().to_string();
        let new_key = new_path.to_string_lossy().to_string();
        let files_changed =
            crate::infra::db::rekey_path_prefix(&mut self.config.ui.file_icons, &old_key, &new_key);
        let folders_changed = crate::infra::db::rekey_path_prefix(
            &mut self.config.ui.folder_icons,
            &old_key,
            &new_key,
        );
        if files_changed || folders_changed {
            utils::save_config(&self.config);
        }
        sender.input(AppMsg::Refresh);
    }
}
