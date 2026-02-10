use crate::model::{AppMsg, FluxApp, SortBy};
use crate::ui::FileItem;
use crate::utils;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Loads the contents of a directory into the application view.
    ///
    /// This method handles directory monitoring, persistent UI settings (sort/icon size),
    /// specialized virtual URI schemes (e.g., `trash://`), and standard filesystem enumeration.
    ///
    /// # Arguments
    /// * `path` - The `PathBuf` representing the directory to load.
    /// * `sender` - Component sender for dispatching asynchronous updates or refresh signals.
    pub fn load_path(&mut self, path: PathBuf, sender: &ComponentSender<Self>) {
        self.directory_monitor = None;
        let path_str = path.to_string_lossy().to_string();

        // 1. Sort Order: Prioritize folder-specific overrides before falling back to defaults
        if let Some(specific_sort) = self.config.ui.folder_sort.get(&path_str) {
            self.sort_by = specific_sort.clone();
        } else {
            self.sort_by = self.config.ui.default_sort.clone();
        }

        // 2. Icon Size: Prioritize folder-specific overrides before falling back to defaults
        if let Some(&size) = self.config.ui.folder_icon_size.get(&path_str) {
            self.current_icon_size = size;
        } else {
            self.current_icon_size = self.config.ui.default_icon_size;
        }

        // --- TRASH HANDLING ---
        // Specialized logic for GIO virtual trash location
        if path_str.starts_with("trash://") {
            self.files.clear();
            let root = gio::File::for_uri(&path_str);

            // Set up directory monitoring for the trash bin
            if let Ok(monitor) =
                root.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            {
                let sender_clone = sender.clone();
                monitor.connect_changed(move |_, _, _, _| {
                    sender_clone.input(AppMsg::Refresh);
                });
                self.directory_monitor = Some(monitor);
            }

            // Enumerate virtual trash children using GIO
            if let Ok(enumerator) = root.enumerate_children(
                "standard::*",
                gio::FileQueryInfoFlags::NONE,
                gio::Cancellable::NONE,
            ) {
                for info in enumerator.flatten() {
                    let name = info.display_name().to_string();
                    let is_dir = info.file_type() == gio::FileType::Directory;
                    let child = root.child(info.name());
                    let child_path = PathBuf::from(child.uri());
                    self.files.append(FileItem {
                        name,
                        icon: info
                            .icon()
                            .unwrap_or_else(|| gio::Icon::for_string("file").unwrap()),
                        thumbnail: None,
                        is_dir,
                        path: child_path,
                        icon_size: self.current_icon_size,
                        is_editing: false,
                    });
                }
            }
            self.current_path = path;
            return;
        }

        // --- STANDARD DIRECTORY HANDLING ---
        let file_obj = gio::File::for_path(&path);

        // Watch for file changes, moves, or deletions in the current directory
        if let Ok(monitor) =
            file_obj.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            let sender_clone = sender.clone();
            monitor.connect_changed(move |_, _, _, _| {
                sender_clone.input(AppMsg::Refresh);
            });
            self.directory_monitor = Some(monitor);
        }

        self.files.clear();

        // Incremental load ID ensures that stale thumbnail/metadata tasks from
        // previous directories do not overwrite current view data.
        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;

        if let Ok(entries) = std::fs::read_dir(&path) {
            let mut items_metadata = Vec::new();

            // Initial pass: Filter hidden files and gather basic metadata
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !self.show_hidden && name.starts_with('.') {
                    continue;
                }

                let target_path = path.join(&name);
                let is_dir = target_path.is_dir();
                let metadata = entry.metadata().ok();
                items_metadata.push((name, target_path, metadata, is_dir));
            }

            // Apply active UI search/filter string
            if !self.filter.is_empty() {
                let query = self.filter.to_lowercase();
                items_metadata.retain(|(name, ..)| name.to_lowercase().contains(&query));
            }

            // Primary Sorting: Directory vs File priority followed by specific SortBy criteria
            let folders_first = self.config.ui.folders_first;
            items_metadata.sort_by(|a, b| {
                if a.3 != b.3 {
                    return if folders_first {
                        b.3.cmp(&a.3)
                    } else {
                        a.3.cmp(&b.3)
                    };
                }
                match self.sort_by {
                    SortBy::Name => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
                    SortBy::Size => {
                        let a_size = a.2.as_ref().map(|m| m.len()).unwrap_or(0);
                        let b_size = b.2.as_ref().map(|m| m.len()).unwrap_or(0);
                        b_size.cmp(&a_size)
                    }
                    SortBy::Date => {
                        let a_time = a.2.as_ref().and_then(|m| m.modified().ok());
                        let b_time = b.2.as_ref().and_then(|m| m.modified().ok());
                        b_time.cmp(&a_time)
                    }
                }
            });

            let mut media_tasks: Vec<(String, PathBuf)> = Vec::new();

            // Build the UI models and identify files requiring background thumbnail generation
            for (name, target_path, _metadata, is_dir) in items_metadata {
                let icon = utils::get_icon_for_path(&target_path, is_dir);
                self.files.append(FileItem {
                    name: name.clone(),
                    icon,
                    thumbnail: None,
                    is_dir,
                    path: target_path.clone(),
                    icon_size: self.current_icon_size,
                    is_editing: false,
                });

                if !is_dir {
                    let (is_img, is_vid) = utils::is_visual_media(&target_path);
                    if is_img || is_vid {
                        if let Ok(abs_path) = target_path.canonicalize() {
                            media_tasks.push((name, abs_path));
                        }
                    }
                }
            }

            self.current_path = path;

            // Offload resource-intensive thumbnail generation to background threads
            self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
        }
    }
}
