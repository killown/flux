use crate::app::state::FluxApp;
use crate::domain::model::{
    AppMsg, ArchiveMsg, FileLoadContext, FileOpsMsg, LoadingMsg, SearchMsg, SortBy, TagsMsg,
};
use crate::infra::loader::get_custom_extension_icon_path;
use crate::infra::loader::is_visual_media_by_ext;
use crate::infra::loader::loader_pool;
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

        // Virtual and special paths are delegated and return immediately.
        if crate::infra::network::is_network_uri(&path) {
            self.archive.locked = false;
            self.current_path = path.clone();
            self.load_network(&path_str, None, sender.clone());
            return;
        }
        if path_str.starts_with(crate::infra::archive::ARCHIVE_URI) {
            if let Some((archive_path, prefix)) =
                crate::infra::archive::parse_archive_uri(&path_str)
            {
                self.current_path = path;
                self.load_archive(archive_path, prefix, None, sender);
            }
            return;
        }

        self.archive.locked = false;

        if path_str.starts_with("recent:///") {
            self.load_recents(sender);
            return;
        }

        if path_str.starts_with("tags://") {
            self.is_loading = false;
            let clean_tag = path_str.trim_start_matches("tags://");
            if clean_tag.is_empty() {
                sender.input(AppMsg::Tags(TagsMsg::ToggleTagPanel));
            } else {
                let tag_query = format!("#{}", clean_tag);
                sender.input(AppMsg::Search(SearchMsg::SwitchHeader(
                    crate::domain::model::constants::VIEW_SEARCH.to_string(),
                )));
                sender.input(AppMsg::Search(SearchMsg::UpdateFilter(tag_query)));
            }
            return;
        }

        if path_str.starts_with("search://") {
            self.is_loading = false;
            sender.input(AppMsg::Search(SearchMsg::ToggleSearchPanel));
            return;
        }

        // Per-folder view state.
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
                            // Deletions are terminal, dispatch immediately.
                            sender_clone.input(AppMsg::FileOps(FileOpsMsg::FileDeleted(path)));
                        }
                        gio::FileMonitorEvent::ChangesDoneHint => {
                            // Writer closed the file after a batch of writes. This
                            // is the signal we trust, dispatch without debouncing.
                            sender_clone.input(AppMsg::FileOps(FileOpsMsg::FileChanged(path)));
                        }
                        gio::FileMonitorEvent::Created
                        | gio::FileMonitorEvent::MovedIn
                        | gio::FileMonitorEvent::Changed => {
                            // Writers that do not emit ChangesDoneHint (some editors,
                            // some network backends) fall back to a debounced Changed
                            // dispatch. Coalesces the write-flush burst that a 30 MB
                            // download emits into a single FileChanged after the file
                            // has stopped growing, so the thumbnail pipeline never runs
                            // against a truncated source.
                            crate::app::update::watcher::debounce_path_event(
                                path,
                                sender_clone.clone(),
                            );
                        }
                        gio::FileMonitorEvent::Moved | gio::FileMonitorEvent::Renamed => {
                            sender_clone.input(AppMsg::FileOps(FileOpsMsg::FileDeleted(path)));
                            if let Some(other) = other_file.and_then(|f| f.path()) {
                                sender_clone.input(AppMsg::FileOps(FileOpsMsg::FileChanged(other)));
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
        let extension_globset = self.filters.extension_globset.clone();
        let expand_labels = self.config.ui.expand_labels;
        let show_empty_dir_emblem = self.config.ui.show_empty_dir_emblem;
        let archives_as_folders = self.config.ui.archives_as_folders;
        let lock_snapshot = self.archive_lock_snapshot();
        let filter = self.filter.clone();
        let mime_filter = self.filters.mime.clone();
        let path_clone = path.clone();
        let sender_clone = sender.clone();
        let auto_list_threshold = self.config.ui.auto_list_threshold;
        let current_list_mode = self.is_list_mode;

        let cache_key = path.canonicalize().unwrap_or_else(|_| path.clone());

        let sender_debounce = sender.clone();
        let session = current_session;
        glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
            sender_debounce.input(AppMsg::Loading(LoadingMsg::ShowLoadingSpinner(session)));
        });

        if !is_trash
            && !path_str.starts_with(crate::infra::archive::ARCHIVE_URI)
            && filter.is_empty()
            && extension_globset.is_none()
            && mime_filter.is_none()
        {
            if let Some(cached) = self.folder_cache.get_mut(&cache_key) {
                crate::hit!("load_path:cache_hit");
                self.is_loading = false;
                cached.last_visited = std::time::Instant::now();
                let cached_items = cached.items.clone();

                // The snapshot may be stale (the folder monitor was cancelled
                // while we were away), so every entry is re-checked. That is
                // one lstat per file, which must not run on the GTK thread:
                // validation, sorting and media-task collection all run on the
                // loader pool, and the result goes through the same
                // `FolderLoaded` path as a cold load. `load_id` makes a newer
                // navigation discard this result.
                relm4::spawn_blocking(move || {
                    let mut cached_items: Vec<FileLoadContext> = loader_pool().install(|| {
                        cached_items
                            .into_par_iter()
                            .filter_map(|mut item| {
                                item.target_path.symlink_metadata().ok()?;
                                // Unlock and forget happen while the snapshot sits in
                                // the cache, so the cached padlock flags are never
                                // trusted. Runs on the pool, not on the GTK thread.
                                lock_snapshot.stamp(&mut item);
                                Some(item)
                            })
                            .collect()
                    });

                    // Keep cached items sorted to the currently active sort configuration
                    loader_pool().install(|| {
                        crate::hit!("load_path:cache_hit:after_sort");
                        cached_items.par_sort_unstable_by(move |a, b| {
                            let group = a.cmp_folder_group(b, folders_first);
                            if group != std::cmp::Ordering::Equal {
                                return group;
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

                    let list_mode =
                        if auto_list_threshold > 0 && cached_items.len() > auto_list_threshold {
                            true
                        } else {
                            current_list_mode
                        };

                    let zip_probe = lock_snapshot.zip_probe_candidates(&cached_items);
                    sender_clone.input(AppMsg::Loading(LoadingMsg::FolderLoaded {
                        path: path_clone,
                        load_id: current_session,
                        items: cached_items,
                        media_tasks,
                        list_mode,
                    }));
                    spawn_zip_lock_probe(zip_probe, current_session, sender_clone);
                });
                return;
            }
        }

        let filter_mode = self.filters.mode;

        relm4::spawn_blocking(move || {
            // Resolve "now" once per load batch. `FileLoadContext::labels()`
            // reads this instead of calling `glib::DateTime::now_local()`
            // per item, which would be O(N) `localtime_r` syscalls on large
            // directories.
            crate::domain::model::set_now_unix(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            );

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

                        if filter_mode == crate::domain::model::FilterMode::OnlyFolders && !is_dir {
                            return None;
                        }
                        if filter_mode == crate::domain::model::FilterMode::OnlyFiles && is_dir {
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

                        if let Some(ref want) = mime_filter {
                            let is_dir_filter = want == crate::domain::model::constants::MIME_DIR;
                            if is_dir_filter {
                                if !is_dir {
                                    return None;
                                }
                            } else if is_dir
                                || crate::utils::media::get_mime_type(&target_path) != *want
                            {
                                return None;
                            }
                        }

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

                        // Visual folder, semantic file: `is_dir` stays false. Runs in
                        // the Rayon pool, so the MIME fallback inside
                        // `is_browsable_archive` never touches the GTK thread.
                        let is_virtual_dir = archives_as_folders
                            && !is_dir
                            && !is_trash
                            && crate::infra::archive::is_browsable_archive(&target_path);

                        let mut item = FileLoadContext::new(
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
                        item.is_virtual_dir = is_virtual_dir;

                        // WARNING: do not remove these four lines.
                        //
                        // These getters are lazy: the first call fills a OnceLock
                        // by asking the filesystem (or, for `labels()`, by doing
                        // locale formatting). If we let them stay empty until the
                        // item reaches the UI, the work happens later, on the GTK
                        // main thread, one item at a time.
                        //
                        // Concretely:
                        //   size()             -> read_dir().count() for directories
                        //   mtime()            -> stat()
                        //   is_foreign_owner() -> stat()
                        //   labels()           -> localtime_r + formatting
                        //
                        // With a batch of ~90 items that's roughly 270 syscalls
                        // plus 90 date-formatting passes serialized inside the
                        // event loop. We call them here, inside the Rayon pool,
                        // so the work runs in parallel on worker threads instead.
                        let _ = item.size();
                        let _ = item.mtime();
                        let _ = item.is_foreign_owner(uid);
                        let _ = item.labels();
                        lock_snapshot.stamp(&mut item);
                        // `is_empty()` does `read_dir().next().is_none()` on the
                        // GTK thread when `show_empty_dir_emblem` is on. Warm it
                        // here, on the Rayon pool, for the same reason as the
                        // getters above.
                        if show_empty_dir_emblem && item.is_dir {
                            let _ = item.is_empty();
                        }

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
                    let group = a.cmp_folder_group(b, folders_first);
                    if group != std::cmp::Ordering::Equal {
                        return group;
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

            let list_mode = if auto_list_threshold > 0 && items.len() > auto_list_threshold {
                true
            } else {
                current_list_mode
            };

            let zip_probe = lock_snapshot.zip_probe_candidates(&items);
            sender_clone.input(AppMsg::Loading(LoadingMsg::FolderLoaded {
                path: path_clone,
                load_id: current_session,
                items,
                media_tasks,
                list_mode,
            }));
            spawn_zip_lock_probe(zip_probe, current_session, sender_clone);
        });
    }
}

/// Reads ZIP headers for the padlock badge after the folder is already on screen.
///
/// Each probe is a few remote range reads on FUSE/cloud mounts (rclone, sshfs),
/// so it must never sit on the load path. Folders without ZIPs spawn nothing.
fn spawn_zip_lock_probe(
    candidates: Vec<PathBuf>,
    load_id: u64,
    sender: AsyncComponentSender<FluxApp>,
) {
    crate::hit!("spawn_zip_lock_probe");
    if candidates.is_empty() {
        return;
    }
    relm4::spawn_blocking(move || {
        let encrypted: Vec<PathBuf> = loader_pool().install(|| {
            candidates
                .into_par_iter()
                .filter(|p| crate::infra::archive::zip_is_encrypted(p))
                .collect()
        });
        if !encrypted.is_empty() {
            sender.input(AppMsg::Archive(ArchiveMsg::ArchiveLocksProbed {
                load_id,
                encrypted,
            }));
        }
    });
}
