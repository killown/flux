use crate::model::{AppMsg, FileLoadContext, FluxApp, SortBy};
use crate::ui::FileItem;
use crate::utils;
use adw::prelude::*;
use gtk::gio;
use rayon::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Synchronizes the application view with the filesystem state at the provided path.
    ///
    /// This method orchestrates a multi-phase pipeline designed to saturate available
    /// CPU cores while minimizing blocking I/O on the main event loop. It leverages
    /// GIO's batch attribute enumeration to reduce context switching and Rayon
    /// for parallel data transformation and sorting.
    ///
    /// # Architecture
    /// 1. **Batch Acquisition**: Retrieves all necessary file attributes in a single kernel request.
    /// 2. **Context Bridging**: Converts non-thread-safe GObjects into a parallelizable domain model.
    /// 3. **Parallel Computation**: Offloads path resolution, canonicalization, and sort-key
    ///    memoization to a background thread pool.
    /// 4. **Tiered Reconciliation**: Updates the UI grid and initiates a prioritized thumbnail
    ///    loading sequence to optimize perceived latency.
    ///
    /// # Arguments
    /// * `path` - The filesystem or virtual URI target (e.g., `trash://`) to enumerate.
    /// * `sender` - Component handle used to dispatch lifecycle updates and background tasks.
    pub fn load_path(&mut self, path: PathBuf, sender: &ComponentSender<Self>) {
        self.directory_monitor = None;
        let path_str = path.to_string_lossy().to_string();

        self.sort_by = self
            .config
            .ui
            .folder_sort
            .get(&path_str)
            .copied()
            .unwrap_or(self.config.ui.default_sort);

        self.current_icon_size = self
            .config
            .ui
            .folder_icon_size
            .get(&path_str)
            .copied()
            .unwrap_or(self.config.ui.default_icon_size);

        let root = if path_str.starts_with("trash://") {
            gio::File::for_uri(&path_str)
        } else {
            gio::File::for_path(&path)
        };

        if let Ok(monitor) =
            root.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            let sender_clone = sender.clone();
            monitor.connect_changed(move |_, _, _, _| {
                sender_clone.input(AppMsg::Refresh);
            });
            self.directory_monitor = Some(monitor);
        }

        self.files.clear();
        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;

        let attributes =
            "standard::name,standard::display-name,standard::type,standard::size,time::modified";

        if let Ok(enumerator) = root.enumerate_children(
            attributes,
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        ) {
            // Extract immutable state from GObjects on the main thread to enable parallel processing.
            let raw_data: Vec<(String, String, bool, u64, i64)> = enumerator
                .flatten()
                .map(|info| {
                    (
                        info.name().to_string_lossy().to_string(),
                        info.display_name().to_string(),
                        info.file_type() == gio::FileType::Directory,
                        info.size() as u64,
                        info.modification_date_time()
                            .map(|dt| dt.to_unix())
                            .unwrap_or(0),
                    )
                })
                .collect();

            let show_hidden = self.show_hidden;
            let sort_strategy = self.sort_by;
            let folders_first = self.config.ui.folders_first;

            // Offload intensive path resolution and sort-key generation to the thread pool.
            let mut items: Vec<FileLoadContext> = raw_data
                .into_par_iter()
                .filter_map(|(name, display_name, is_dir, size, mtime)| {
                    if !show_hidden && name.starts_with('.') {
                        return None;
                    }

                    let target_path = if path_str.starts_with("trash://") {
                        PathBuf::from(root.child(&name).uri())
                    } else {
                        path.join(&name)
                    };

                    let mut thumbnail_path = None;
                    if !is_dir {
                        let (is_img, is_vid) = utils::is_visual_media(&target_path);
                        if is_img || is_vid {
                            thumbnail_path = target_path.canonicalize().ok();
                        }
                    }

                    Some(FileLoadContext {
                        sort_name: display_name.to_lowercase(),
                        display_name,
                        target_path,
                        size,
                        mtime,
                        is_dir,
                        thumbnail_path,
                    })
                })
                .collect();

            if !self.filter.is_empty() {
                let query = self.filter.to_lowercase();
                items.retain(|item| item.sort_name.contains(&query));
            }

            items.par_sort_unstable_by(move |a, b| {
                if a.is_dir != b.is_dir {
                    return if folders_first {
                        b.is_dir.cmp(&a.is_dir)
                    } else {
                        a.is_dir.cmp(&b.is_dir)
                    };
                }
                match sort_strategy {
                    SortBy::Name => a.sort_name.cmp(&b.sort_name),
                    SortBy::Size => b.size.cmp(&a.size),
                    SortBy::Date => b.mtime.cmp(&a.mtime),
                }
            });

            let mut media_tasks = Vec::new();
            for item in items {
                let icon = utils::get_icon_for_path(&item.target_path, item.is_dir);

                self.files.append(FileItem {
                    name: item.display_name.clone(),
                    icon,
                    thumbnail: None,
                    is_dir: item.is_dir,
                    path: item.target_path,
                    icon_size: self.current_icon_size,
                    is_editing: false,
                });

                if let Some(abs_path) = item.thumbnail_path {
                    media_tasks.push((item.display_name, abs_path));
                }
            }

            self.current_path = path;

            // Prioritize initial thumbnail generation to improve perceived UI readiness.
            let priority_limit = 15;
            if media_tasks.len() > priority_limit {
                let background_tasks = media_tasks.split_off(priority_limit);
                self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
                self.spawn_thumbnail_loader(background_tasks, current_session, sender.clone());
            } else {
                self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
            }
        }
    }
}
