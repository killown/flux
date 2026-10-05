use super::icons::get_custom_extension_icon_path;
use super::media::is_visual_media_by_ext;
use super::pool::loader_pool;
use crate::model::{AppMsg, FileLoadContext, FluxApp, SortBy};
use crate::utils::media::is_audio_file;
use adw::prelude::*;
use gtk::gio;
use ignore::{ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};
use rayon::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

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
        crate::hit!("load_path");
        let path_str = path.to_string_lossy().to_string();

        // ── Virtual / special paths - delegate and return immediately ────────────
        if crate::services::network::is_network_uri(&path) {
            self.archive_locked = false;
            self.current_path = path.clone();
            self.load_network(&path_str, None, sender.clone());
            return;
        }
        if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            if let Some((archive_path, prefix)) =
                crate::services::archive::parse_archive_uri(&path_str)
            {
                self.current_path = path;
                self.load_archive(archive_path, prefix, None, sender);
            }
            return;
        }

        // Exiting or not inside an archive: clear the lock state
        self.archive_locked = false;

        if path_str.starts_with("recent:///") {
            self.load_recents(sender);
            return;
        }

        if path_str.starts_with("tags://") {
            self.is_loading = false;
            let clean_tag = path_str.trim_start_matches("tags://");
            if clean_tag.is_empty() {
                sender.input(AppMsg::ToggleTagPanel);
            } else {
                let tag_query = format!("#{}", clean_tag);
                sender.input(AppMsg::SwitchHeader(
                    crate::ui::constants::VIEW_SEARCH.to_string(),
                ));
                sender.input(AppMsg::UpdateFilter(tag_query));
            }
            return;
        }

        if path_str.starts_with("search://") {
            self.is_loading = false;
            sender.input(AppMsg::ToggleSearchPanel);
            return;
        }

        // ── Persistent per-folder view state ─────────────────────────────────────
        let mut folders_first = self
            .config
            .ui
            .current_folders_first
            .get(&path_str)
            .copied()
            .unwrap_or(self.config.ui.folders_first);

        if let Ok(Some((sort, rev, size, ff))) = self.state_db.get_view(&path) {
            self.sort_by = match sort.as_str() {
                "Date" => SortBy::Date,
                "Size" => SortBy::Size,
                "Type" => SortBy::Type,
                _ => SortBy::Name,
            };
            self.sort_ascending = !rev;
            self.current_icon_size = size as i32;

            // If current_folders_first has an explicit override, prioritize it,
            // otherwise use the state_db value.
            if !self.config.ui.current_folders_first.contains_key(&path_str) {
                folders_first = ff;
            }
        } else {
            self.sort_by = self.config.ui.default_sort;
            self.sort_ascending = true;
            self.current_icon_size = self.config.ui.default_icon_size;
        }

        // ── Folder monitor ────────────────────────────────────────────────────────
        if let Some(old_mon) = self.directory_monitor.take() {
            old_mon.cancel();
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

        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.pending_thumbnails.clear();
        let tm = self.thumbnail_manager.clone();
        relm4::spawn(async move {
            tm.clear_and_cancel_all();
        });

        let show_hidden = self.show_hidden;
        let sort_strategy = self.sort_by;
        let sort_ascending = self.sort_ascending;
        let is_trash = path_str.starts_with("trash://");
        let config_folder_icons = self.state_db.load_folder_icons();
        self.config.ui.folder_icons = config_folder_icons.clone();
        let config_file_icons = self.config.ui.file_icons.clone();
        let extension_globset = self.extension_globset.clone();
        let expand_labels = self.config.ui.expand_labels;
        let filter = self.filter.clone();
        let path_clone = path.clone();
        let sender_clone = sender.clone();

        let cache_key = path.canonicalize().unwrap_or_else(|_| path.clone());

        if !is_trash
            && !path_str.starts_with(crate::services::archive::ARCHIVE_URI)
            && filter.is_empty()
            && extension_globset.is_none()
        {
            if let Some(cached) = self.folder_cache.get_mut(&cache_key) {
                self.is_loading = false;
                cached
                    .items
                    .retain(|item| item.target_path.symlink_metadata().is_ok());
                cached
                    .media_tasks
                    .retain(|(_, p)| p.symlink_metadata().is_ok());
                cached.last_visited = std::time::Instant::now();
                let mut cached_items = cached.items.clone();

                // Keep cached items sorted to the currently active sort configuration
                loader_pool().install(|| {
                    cached_items.par_sort_unstable_by(move |a, b| {
                        if a.is_dir != b.is_dir {
                            return if folders_first {
                                b.is_dir.cmp(&a.is_dir)
                            } else {
                                a.is_dir.cmp(&b.is_dir)
                            };
                        }

                        // In cached folder sort comparator:
                        let primary_order = match sort_strategy {
                            SortBy::Name => a.sort_name.cmp(&b.sort_name),
                            SortBy::Size => a.size().cmp(&b.size()),
                            SortBy::Date => a.mtime().cmp(&b.mtime()),
                            SortBy::Type => a.sort_ext.cmp(&b.sort_ext),
                        };

                        let tie_breaker = if primary_order == std::cmp::Ordering::Equal {
                            a.sort_name.cmp(&b.sort_name)
                        } else {
                            primary_order
                        };

                        if sort_ascending {
                            tie_breaker
                        } else {
                            tie_breaker.reverse()
                        }
                    })
                });

                let media_tasks: Vec<(u32, PathBuf)> = cached_items
                    .iter()
                    .enumerate()
                    .filter_map(|(i, item)| {
                        if item.is_dir {
                            return None;
                        }
                        let source = item
                            .custom_icon
                            .as_ref()
                            .map(PathBuf::from)
                            .or_else(|| item.thumbnail_path.clone())?;
                        Some((i as u32, source))
                    })
                    .collect();

                self.handle_folder_loaded(path, current_session, cached_items, media_tasks, sender);
                return;
            }
        }

        // Schedule delayed spinner activation only when performing a cold background read
        let sender_debounce = sender.clone();
        let session = current_session;
        glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
            sender_debounce.input(AppMsg::ShowLoadingSpinner(session));
        });

        // ── Fast asynchronous item loader ─────────────────────────────────────────
        relm4::spawn_blocking(move || {
            // Fast directory reading without individual stat() calls per file
            let raw_entries: Vec<(String, bool, bool)> = if is_trash {
                let root_bg = gio::File::for_uri(&path_clone.to_string_lossy());
                if let Ok(enumerator) = root_bg.enumerate_children(
                    "standard::name,standard::type,standard::size,standard::is-symlink",
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                ) {
                    enumerator
                        .flatten()
                        .map(|info| {
                            (
                                info.name().to_string_lossy().to_string(),
                                info.file_type() == gio::FileType::Directory,
                                info.is_symlink(),
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                let mut builder = WalkBuilder::new(&path_clone);
                builder
                    .hidden(false)
                    .parents(false)
                    .ignore(false)
                    .git_ignore(false)
                    .max_depth(Some(1))
                    .follow_links(false);

                let walker = builder.build_parallel();
                let (tx, rx) = mpsc::channel::<(String, bool, bool)>();

                struct EntryVisitor {
                    tx: mpsc::Sender<(String, bool, bool)>,
                }

                impl ParallelVisitor for EntryVisitor {
                    fn visit(
                        &mut self,
                        result: Result<ignore::DirEntry, ignore::Error>,
                    ) -> WalkState {
                        let entry = match result {
                            Ok(e) => e,
                            Err(_) => return WalkState::Continue,
                        };

                        if entry.depth() == 0 {
                            return WalkState::Continue;
                        }

                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_symlink = entry.path_is_symlink();

                        // WARNING: changing this could cause bugs:
                        // entry.file_type() reports the type of the symlink file itself,
                        // not the directory it points to. Without checking `is_symlink && entry.path().is_dir()`,
                        // symlink folders will be treated as regular files, breaking directory navigation.
                        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                            || (is_symlink && entry.path().is_dir());

                        let _ = self.tx.send((name, is_dir, is_symlink));
                        WalkState::Continue
                    }
                }

                struct EntryVisitorBuilder {
                    tx: mpsc::Sender<(String, bool, bool)>,
                }

                impl<'s> ParallelVisitorBuilder<'s> for EntryVisitorBuilder {
                    fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
                        Box::new(EntryVisitor {
                            tx: self.tx.clone(),
                        })
                    }
                }

                let mut visitor_builder = EntryVisitorBuilder { tx };
                walker.visit(&mut visitor_builder);
                drop(visitor_builder);

                rx.into_iter().collect()
            };
            let is_inside_thumb_cache = dirs::cache_dir()
                .map(|c| path_clone.starts_with(c.join("thumbnails")))
                .unwrap_or(false);

            let mut items: Vec<FileLoadContext> = loader_pool().install(|| {
                let uid = unsafe { libc::geteuid() };
                raw_entries
                    .into_par_iter()
                    .filter_map(|(name, is_dir, is_symlink)| {
                        if !show_hidden && name.starts_with('.') {
                            return None;
                        }

                        if !is_dir {
                            if let Some(ref gs) = extension_globset {
                                if !gs.is_match(name.to_lowercase()) {
                                    return None;
                                }
                            }
                        }

                        let target_path = if is_trash {
                            PathBuf::from(format!("trash:///{}", name))
                        } else {
                            path_clone.join(&name)
                        };

                        let mut thumbnail_path = None;
                        if !is_dir && !is_inside_thumb_cache {
                            let (is_img, is_vid) = is_visual_media_by_ext(&target_path);
                            let is_exe = target_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
                            let is_audio = is_audio_file(&target_path);

                            if is_img || is_vid || is_exe || is_audio {
                                thumbnail_path = Some(target_path.clone());
                            }
                        }

                        let sort_name = name.to_lowercase();
                        let sort_ext = std::path::Path::new(&name)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_ascii_lowercase())
                            .unwrap_or_default();

                        let custom_icon = if config_file_icons.is_empty()
                            && (!is_dir || config_folder_icons.is_empty())
                        {
                            if !is_dir && !sort_ext.is_empty() {
                                get_custom_extension_icon_path(&sort_ext)
                                    .map(|p| p.to_string_lossy().into_owned())
                            } else {
                                None
                            }
                        } else {
                            let path_key = target_path.to_str();
                            path_key.and_then(|k| {
                                config_file_icons.get(k).cloned().or_else(|| {
                                    if is_dir {
                                        config_folder_icons.get(k).cloned()
                                    } else if !sort_ext.is_empty() {
                                        get_custom_extension_icon_path(&sort_ext)
                                            .map(|p| p.to_string_lossy().into_owned())
                                    } else {
                                        None
                                    }
                                })
                            })
                        };

                        let is_broken_symlink = if is_symlink {
                            std::fs::metadata(&target_path).is_err()
                        } else {
                            false
                        };

                        let item = FileLoadContext::new(
                            name,
                            target_path,
                            is_dir,
                            sort_name,
                            sort_ext,
                            thumbnail_path,
                            expand_labels,
                            custom_icon,
                            is_symlink,
                            is_broken_symlink,
                        );

                        // WARNING: do not remove these three lines.
                        //
                        // These getters are lazy: the first call fills a OnceLock
                        // by asking the filesystem. If we let them stay empty until
                        // the item reaches the UI, the syscalls happen later, on
                        // the GTK main thread, one item at a time.
                        //
                        // Concretely:
                        //   size()             -> read_dir().count() for directories
                        //   mtime()            -> stat()
                        //   is_foreign_owner() -> stat()
                        //
                        // With a batch of ~90 items that's roughly 180 syscalls
                        // serialized inside the event loop. Before this fix each
                        // FolderLoadedChunk spent ~40ms on that, which made folder
                        // loads noticeably heavier. We call them here, inside the
                        // Rayon pool, so the syscalls run in parallel on worker
                        // threads instead.
                        let _ = item.size();
                        let _ = item.mtime();
                        let _ = item.is_foreign_owner(uid);

                        Some(item)
                    })
                    .collect()
            });
            if !filter.is_empty() {
                let query = filter.to_lowercase();
                items.retain(|item| crate::utils::search::fuzzy_match(&item.sort_name, &query));
            }

            // Sort - run inside the capped pool so no new threads are spawned
            loader_pool().install(|| {
                items.par_sort_unstable_by(move |a, b| {
                    if a.is_dir != b.is_dir {
                        return if folders_first {
                            b.is_dir.cmp(&a.is_dir)
                        } else {
                            a.is_dir.cmp(&b.is_dir)
                        };
                    }

                    let primary_order = match sort_strategy {
                        SortBy::Name => a.sort_name.cmp(&b.sort_name),
                        SortBy::Size => a.size().cmp(&b.size()),
                        SortBy::Date => a.mtime().cmp(&b.mtime()),
                        SortBy::Type => a.sort_ext.cmp(&b.sort_ext),
                    };

                    let tie_breaker = if primary_order == std::cmp::Ordering::Equal {
                        a.sort_name.cmp(&b.sort_name)
                    } else {
                        primary_order
                    };

                    if sort_ascending {
                        tie_breaker
                    } else {
                        tie_breaker.reverse()
                    }
                });
            });

            let media_tasks: Vec<(u32, PathBuf)> = items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    if item.is_dir {
                        return None;
                    }
                    let source = item
                        .custom_icon
                        .as_ref()
                        .map(PathBuf::from)
                        .or_else(|| item.thumbnail_path.clone())?;
                    Some((i as u32, source))
                })
                .collect();

            sender_clone.input(AppMsg::FolderLoaded {
                path: path_clone,
                load_id: current_session,
                items,
                media_tasks,
            });
        });
    }
}
