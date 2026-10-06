use crate::model::{AppMsg, FluxApp};
use crate::ui::FileItem;
use crate::utils;
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_file_deleted(&mut self, path: PathBuf) {
        crate::services::search::indexer::notify_deleted(&path);

        if let Some(parent) = path.parent() {
            self.folder_cache.remove(parent);
        }
        self.folder_cache.remove(&path);

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

    pub fn handle_file_changed(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_file_changed");
        if let Some(parent) = path.parent() {
            self.folder_cache.remove(parent);
        }
        self.folder_cache.remove(&path);

        if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) {
            let file = gio::File::for_path(&path);
            let attributes = "standard::name,standard::display-name,standard::type";

            if let Ok(info) = file.query_info(
                attributes,
                gio::FileQueryInfoFlags::NONE,
                gio::Cancellable::NONE,
            ) {
                let is_dir = info.file_type() == gio::FileType::Directory;
                let display_name = info.display_name().to_string();
                let icon = utils::icon::get_icon_for_path(&path, is_dir);

                let target_idx = (0..self.tabs[self.active_tab_index].files.len()).find(|&i| {
                    self.tabs[self.active_tab_index]
                        .files
                        .get(i)
                        .is_some_and(|r| r.borrow().name == name)
                });
                if let Some(idx) = target_idx {
                    let item_opt = self.tabs[self.active_tab_index]
                        .files
                        .get(idx)
                        .map(|w| w.borrow().clone());
                    if let Some(mut item) = item_opt {
                        item.icon = icon;
                        self.tabs[self.active_tab_index].files.remove(idx);
                        self.tabs[self.active_tab_index].files.insert(idx, item);
                    }
                } else {
                    crate::services::search::indexer::notify_created(&path);

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

                    let current_session = self.load_id.load(Ordering::SeqCst);
                    self.spawn_thumbnail_loader(
                        vec![(grid_idx, path.clone())],
                        current_session,
                        self.active_tab_index,
                        sender.clone(),
                    );
                    sender.input(AppMsg::Refresh);
                }
            } else {
                crate::services::search::indexer::notify_deleted(&path);

                // File no longer exists, treat as deleted, remove from grid directly
                let target_idx = (0..self.tabs[self.active_tab_index].files.len()).find(|&i| {
                    self.tabs[self.active_tab_index]
                        .files
                        .get(i)
                        .is_some_and(|r| r.borrow().name == name)
                });
                if let Some(idx) = target_idx {
                    self.tabs[self.active_tab_index].files.remove(idx);
                }
            }
        }
    }

    pub fn handle_start_rename(&mut self, path: PathBuf) {
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
        if let Some(path) = self.get_selected_path() {
            sender.input(AppMsg::StartRename(path));
        }
    }

    pub fn handle_file_deleted_dispatch(&mut self, path: PathBuf) {
        crate::hit!("handle_file_deleted");
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
        crate::services::search::indexer::notify_moved(&old_path, &new_path);

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
        let files_changed = crate::services::db::rekey_path_prefix(
            &mut self.config.ui.file_icons,
            &old_key,
            &new_key,
        );
        let folders_changed = crate::services::db::rekey_path_prefix(
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
