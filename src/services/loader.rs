use crate::model::{AppMsg, FileLoadContext, FluxApp, SortBy};
use crate::ui::constants;
use crate::ui::FileItem;
use crate::utils;
use adw::prelude::*;
use gtk::gdk;
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
    pub fn load_path(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        self.directory_monitor = None;
        let path_str = path.to_string_lossy().to_string();

        let mut folders_first = self.config.ui.folders_first;

        // Load persistent folder state from SQLite before scanning
        if let Ok(Some((sort, _rev, size, ff))) = self.state_db.get_view(&path) {
            self.sort_by = match sort.as_str() {
                "Date" => SortBy::Date,
                "Size" => SortBy::Size,
                _ => SortBy::Name,
            };
            self.current_icon_size = size as i32;
            folders_first = ff;
        } else {
            self.sort_by = self.config.ui.default_sort;
            self.current_icon_size = self.config.ui.default_icon_size;
        }

        let root = if path_str.starts_with("trash://") {
            gio::File::for_uri(&path_str)
        } else {
            gio::File::for_path(&path)
        };

        if let Ok(monitor) =
            root.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            let sender_clone = sender.clone();
            monitor.connect_changed(move |_, file, other_file, event| {
                if let Some(path) = file.path() {
                    match event {
                        gio::FileMonitorEvent::Deleted | gio::FileMonitorEvent::MovedOut => {
                            sender_clone.input(AppMsg::FileDeleted(path));
                        }
                        gio::FileMonitorEvent::Created
                        | gio::FileMonitorEvent::MovedIn
                        | gio::FileMonitorEvent::Changed
                        | gio::FileMonitorEvent::ChangesDoneHint => {
                            sender_clone.input(AppMsg::FileChanged(path));
                        }
                        gio::FileMonitorEvent::Moved | gio::FileMonitorEvent::Renamed => {
                            sender_clone.input(AppMsg::FileDeleted(path));
                            if let Some(other) = other_file.and_then(|f| f.path()) {
                                sender_clone.input(AppMsg::FileChanged(other));
                            }
                        }
                        _ => {}
                    }
                }
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

            // Resolve the cache directory for the specific size defined in constants.
            let cache_base = dirs::cache_dir().unwrap_or_default().join("thumbnails");
            let thumb_folder = match constants::CACHED_THUMBNAIL_SIZE {
                512 => "xx-large",
                256 => "x-large",
                _ => "normal",
            };
            let target_cache_dir = cache_base.join(thumb_folder);

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
                            let uri = format!("file://{}", target_path.to_string_lossy());
                            let hash = format!("{:x}", md5::compute(uri));
                            let cached = target_cache_dir.join(format!("{}.png", hash));

                            // Check for instant cache hit at the configured hi-res size.
                            if cached.exists() {
                                thumbnail_path = Some(cached);
                            } else {
                                thumbnail_path = Some(target_path.clone());
                            }
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
                        b.is_dir.cmp(&a.is_dir) // Directories first
                    } else {
                        a.is_dir.cmp(&b.is_dir) // Files first or mixed (depending on strategy)
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

                // Load the texture immediately if it exists in the hi-res cache.
                let mut instant_thumb = None;
                if let Some(ref tp) = item.thumbnail_path {
                    if tp.starts_with(&cache_base) {
                        instant_thumb = gdk::Texture::from_file(&gio::File::for_path(tp)).ok();
                    }
                }

                self.files.append(FileItem {
                    name: item.display_name.clone(),
                    icon,
                    thumbnail: instant_thumb,
                    is_dir: item.is_dir,
                    path: item.target_path,
                    icon_size: self.current_icon_size,
                    size: item.size,
                    is_editing: false,
                });

                if let Some(abs_path) = item.thumbnail_path {
                    // Only dispatch to background if not already loaded from the high-res cache.
                    if !abs_path.starts_with(&cache_base) {
                        media_tasks.push((item.display_name, abs_path));
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileLoadContext;
    use std::path::{Path, PathBuf};

    // Helper to create a mock FileLoadContext
    fn mock_ctx(name: &str, is_dir: bool, size: u64, mtime: i64) -> FileLoadContext {
        FileLoadContext {
            display_name: name.to_string(),
            sort_name: name.to_lowercase(),
            target_path: PathBuf::from(name),
            size,
            mtime,
            is_dir,
            thumbnail_path: None,
        }
    }

    #[test]
    fn test_sort_by_name() {
        let mut items = vec![
            mock_ctx("Zebra", false, 0, 0),
            mock_ctx("Apple", false, 0, 0),
            mock_ctx("Banana", false, 0, 0),
        ];

        items.par_sort_unstable_by(|a, b| {
            // Simplified sort logic for testing Name only
            a.sort_name.cmp(&b.sort_name)
        });

        assert_eq!(items[0].display_name, "Apple");
        assert_eq!(items[1].display_name, "Banana");
        assert_eq!(items[2].display_name, "Zebra");
    }

    #[test]
    fn test_sort_by_size_descending() {
        let mut items = vec![
            mock_ctx("Small", false, 100, 0),
            mock_ctx("Large", false, 1000, 0),
            mock_ctx("Medium", false, 500, 0),
        ];

        items.par_sort_unstable_by(|a, b| {
            b.size.cmp(&a.size) // Descending
        });

        assert_eq!(items[0].display_name, "Large");
        assert_eq!(items[1].display_name, "Medium");
        assert_eq!(items[2].display_name, "Small");
    }

    #[test]
    fn test_sort_by_date_descending() {
        let mut items = vec![
            mock_ctx("Old", false, 0, 1000),
            mock_ctx("New", false, 0, 3000),
            mock_ctx("Mid", false, 0, 2000),
        ];

        items.par_sort_unstable_by(|a, b| {
            b.mtime.cmp(&a.mtime) // Descending
        });

        assert_eq!(items[0].display_name, "New");
        assert_eq!(items[1].display_name, "Mid");
        assert_eq!(items[2].display_name, "Old");
    }

    #[test]
    fn test_folders_first_sorting() {
        let mut items = vec![
            mock_ctx("file.txt", false, 0, 0),
            mock_ctx("DirA", true, 0, 0),
            mock_ctx("file2.txt", false, 0, 0),
            mock_ctx("DirB", true, 0, 0),
        ];

        // Simulate the folders_first logic from loader.rs
        let folders_first = true;
        items.par_sort_unstable_by(|a, b| {
            if a.is_dir != b.is_dir {
                return if folders_first {
                    b.is_dir.cmp(&a.is_dir) // Directories first (true > false)
                } else {
                    a.is_dir.cmp(&b.is_dir)
                };
            }
            a.sort_name.cmp(&b.sort_name)
        });

        assert!(items[0].is_dir);
        assert!(items[1].is_dir);
        assert!(!items[2].is_dir);
        assert!(!items[3].is_dir);

        assert_eq!(items[0].display_name, "DirA");
        assert_eq!(items[1].display_name, "DirB");
        assert_eq!(items[2].display_name, "file.txt");
        assert_eq!(items[3].display_name, "file2.txt");
    }

    #[test]
    fn test_files_first_sorting() {
        let mut items = vec![
            mock_ctx("file.txt", false, 0, 0),
            mock_ctx("DirA", true, 0, 0),
        ];

        let folders_first = false;
        items.par_sort_unstable_by(|a, b| {
            if a.is_dir != b.is_dir {
                return if folders_first {
                    b.is_dir.cmp(&a.is_dir)
                } else {
                    a.is_dir.cmp(&b.is_dir) // Files first (false < true)
                };
            }
            a.sort_name.cmp(&b.sort_name)
        });

        assert!(!items[0].is_dir);
        assert!(items[1].is_dir);
    }

    #[test]
    fn test_hidden_file_filtering() {
        let show_hidden = false;
        let raw_data = [
            ("visible.txt", false, 0, 0),
            (".hidden.txt", false, 0, 0),
            ("another.txt", false, 0, 0),
        ];

        let filtered: Vec<_> = raw_data
            .iter()
            .filter_map(|(name, is_dir, size, mtime)| {
                if !show_hidden && name.starts_with('.') {
                    return None;
                }
                Some(mock_ctx(name, *is_dir, *size, *mtime))
            })
            .collect();

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].display_name, "visible.txt");
        assert_eq!(filtered[1].display_name, "another.txt");
    }

    #[test]
    fn test_thumbnail_cache_logic() {
        let cache_base = PathBuf::from(".cache/thumbnails");
        let thumb_folder = "normal";
        let target_cache_dir = cache_base.join(thumb_folder);

        let file_path = Path::new("Pictures/photo.jpg");
        let uri = format!("file://{}", file_path.to_string_lossy());
        let hash = format!("{:x}", md5::compute(uri));
        let cached = target_cache_dir.join(format!("{}.png", hash));

        // If cached exists, thumbnail_path should be Some(cached)
        // If not, it should be Some(file_path) to trigger generation
        // This test verifies the path construction is correct

        assert!(cached.starts_with(&target_cache_dir));
        assert!(cached.to_string_lossy().ends_with(".png"));
    }
}
