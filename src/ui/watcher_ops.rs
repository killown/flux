use crate::model::{AppMsg, FluxApp};
use crate::ui::FileItem;
use crate::utils;
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_file_deleted(&mut self, path: PathBuf) {
        if self.is_content_searching {
            // A file can have multiple result rows (one per matching line),
            // remove all of them
            let mut i = 0;
            while i < self.files.len() {
                if self.files.get(i).is_some_and(|r| r.borrow().path == path) {
                    self.files.remove(i);
                    // don't increment i, next item shifts into this slot
                } else {
                    i += 1;
                }
            }
            return;
        }

        if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) {
            let target_idx = (0..self.files.len())
                .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
            if let Some(idx) = target_idx {
                self.files.remove(idx);
            }
        }
    }

    pub fn handle_file_changed(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
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
                let icon = utils::get_icon_for_path(&path, is_dir);

                let target_idx = (0..self.files.len())
                    .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
                if let Some(idx) = target_idx {
                    if let Some(item_wrapper) = self.files.get(idx) {
                        let mut item = item_wrapper.borrow().clone();
                        item.icon = icon;
                        self.files.remove(idx);
                        self.files.insert(idx, item);
                    }
                } else {
                    let item = FileItem {
                        name: display_name.clone(),
                        icon,
                        thumbnail: None,
                        is_dir,
                        path: path.clone(),
                        icon_size: if self.is_list_mode {
                            self.current_list_icon_size
                        } else {
                            self.current_icon_size
                        },
                        size: info.size() as u64,
                        mtime: info
                            .modification_date_time()
                            .map(|dt| dt.to_unix())
                            .unwrap_or(0),
                        is_editing: false,
                        is_foreign_owner: false,
                        expand_labels: self.config.ui.expand_labels,
                        is_list_mode: self.is_list_mode,
                        is_custom_icon: false,
                        active_path: Rc::new(RefCell::new(None)),
                    };
                    self.files.append(item);

                    let current_session = self.load_id.load(Ordering::SeqCst);
                    self.spawn_thumbnail_loader(
                        vec![(self.files.len() - 1, path)],
                        current_session,
                        sender.clone(),
                    );
                    sender.input(AppMsg::Refresh);
                }
            } else {
                let target_idx = (0..self.files.len())
                    .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
                if let Some(idx) = target_idx {
                    self.files.remove(idx);
                }
            }
        }
    }

    pub fn handle_start_rename(&mut self, path: PathBuf) {
        let target_idx = (0..self.files.len())
            .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().path == path));

        if let Some(idx) = target_idx {
            if let Some(item_wrapper) = self.files.get(idx) {
                let mut item = item_wrapper.borrow().clone();
                item.is_editing = true;
                self.files.remove(idx);
                self.files.insert(idx, item);
            }
        }
    }

    pub fn handle_trigger_rename_selection(&mut self, sender: &AsyncComponentSender<Self>) {
        if let Some(path) = self.get_selected_path() {
            sender.input(AppMsg::StartRename(path));
        }
    }
}
