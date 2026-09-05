use crate::model::{AppMsg, FileLoadContext, FluxApp, SortBy};
use crate::services::archive;
use crate::ui::FileItem;
use crate::utils;
use adw::prelude::*;
use gtk::gio;
use rayon::prelude::*;
use relm4::prelude::*;
use std::cell::RefCell;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;

/// Shared Rayon thread pool for directory listing work.
///
/// Capped at 4 threads with 2 MiB stacks instead of Rayon's global default
/// (one thread per CPU core, 8 MiB stacks each). This prevents the one-time
/// ~50 MiB RSS jump that occurs when the global pool fully commits its stacks
/// on the first large directory load.
static LOADER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn loader_pool() -> &'static rayon::ThreadPool {
    LOADER_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .stack_size(2 * 1024 * 1024)
            .build()
            .expect("failed to build loader thread pool")
    })
}

/// Returns `(is_image, is_video)` by extension only - zero I/O, safe to call inside
/// a Rayon iterator on the directory listing hot path. Accuracy is intentionally
/// approximate: false positives are harmless (they go into `media_tasks` and get
/// filtered again by the full `is_visual_media` check inside `get_or_create_thumbnail`).
#[inline]
fn is_visual_media_by_ext(path: &std::path::Path) -> (bool, bool) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some(
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tiff"
            | "tif" | "jxl" | "svg" | "pdf" | "ttf" | "otf" | "woff" | "woff2" | "ttc",
        ) => (true, false),
        Some(
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts"
            | "ogv",
        ) => (false, true),
        _ => (false, false),
    }
}

/// Returns the path to use as thumbnail source, giving priority to custom icon.
#[allow(dead_code)]
pub fn resolve_thumb_source(ctx: &FileLoadContext) -> Option<PathBuf> {
    ctx.custom_icon
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| ctx.thumbnail_path.clone())
}

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
        unsafe {
            libc::malloc_trim(0);
        }
        self.is_loading = true;
        let path_str = path.to_string_lossy().to_string();

        // ── Virtual / special paths - delegate and return immediately ────────────
        if crate::services::network::is_network_uri(&path) {
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
        if path_str.starts_with("recent:///") {
            self.load_recents(sender);
            return;
        }

        // ── Persistent per-folder view state ─────────────────────────────────────
        let mut folders_first = self.config.ui.folders_first;
        if let Ok(Some((sort, rev, size, ff))) = self.state_db.get_view(&path) {
            self.sort_by = match sort.as_str() {
                "Date" => SortBy::Date,
                "Size" => SortBy::Size,
                "Type" => SortBy::Type,
                _ => SortBy::Name,
            };
            self.sort_ascending = !rev;
            self.current_icon_size = size as i32;
            folders_first = ff;
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

        if !is_trash
            && !path_str.starts_with(crate::services::archive::ARCHIVE_URI)
            && filter.is_empty()
            && extension_globset.is_none()
        {
            if let Some(cached) = self.folder_cache.get_mut(&path) {
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

                        let primary_order = match sort_strategy {
                            SortBy::Name => a.sort_name.cmp(&b.sort_name),
                            SortBy::Size => a.size.cmp(&b.size),
                            SortBy::Date => a.mtime.cmp(&b.mtime),
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

        // ── Fast asynchronous item loader ─────────────────────────────────────────
        relm4::spawn_blocking(move || {
            let current_uid = unsafe { libc::geteuid() };

            // Fast directory reading without individual stat() calls per file
            let raw_entries: Vec<(String, bool)> = if is_trash {
                let root_bg = gio::File::for_uri(&path_clone.to_string_lossy());
                if let Ok(enumerator) = root_bg.enumerate_children(
                    "standard::name,standard::type,standard::size",
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                ) {
                    enumerator
                        .flatten()
                        .map(|info| {
                            (
                                info.name().to_string_lossy().to_string(),
                                info.file_type() == gio::FileType::Directory,
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                match std::fs::read_dir(&path_clone) {
                    Ok(read_dir) => read_dir
                        .flatten()
                        .map(|entry| {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let is_dir = entry.path().is_dir();
                            (name, is_dir)
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                }
            };

            let mut items: Vec<FileLoadContext> = loader_pool().install(|| {
                raw_entries
                    .into_par_iter()
                    .filter_map(|(name, is_dir)| {
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

                        let (size, mtime, is_foreign_owner) = target_path
                            .metadata()
                            .ok()
                            .map(|m| {
                                let s = if is_dir { 0 } else { m.len() };
                                let t = m
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs() as i64)
                                    .unwrap_or(0);
                                let foreign = m.uid() != current_uid;
                                (s, t, foreign)
                            })
                            .unwrap_or((0, 0, false));

                        let mut thumbnail_path = None;
                        if !is_dir {
                            let (is_img, is_vid) = is_visual_media_by_ext(&target_path);
                            if is_img || is_vid {
                                thumbnail_path = Some(target_path.clone());
                            }
                        }

                        let custom_icon = if config_file_icons.is_empty()
                            && (!is_dir || config_folder_icons.is_empty())
                        {
                            None
                        } else {
                            let path_key = target_path.to_str();
                            path_key.and_then(|k| {
                                config_file_icons.get(k).cloned().or_else(|| {
                                    if is_dir {
                                        config_folder_icons.get(k).cloned()
                                    } else {
                                        None
                                    }
                                })
                            })
                        };

                        let sort_name = name.to_lowercase();
                        let sort_ext = std::path::Path::new(&name)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_ascii_lowercase())
                            .unwrap_or_default();

                        Some(FileLoadContext {
                            sort_name,
                            sort_ext,
                            display_name: name,
                            target_path,
                            size,
                            mtime,
                            is_dir,
                            thumbnail_path,
                            is_foreign_owner,
                            expand_labels,
                            custom_icon,
                        })
                    })
                    .collect()
            });

            if !filter.is_empty() {
                let query = filter.to_lowercase();
                items.retain(|item| item.sort_name.contains(&query));
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
                        SortBy::Size => a.size.cmp(&b.size),
                        SortBy::Date => a.mtime.cmp(&b.mtime),
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

    /// Asynchronously lists a network location via GVFS and dispatches the result.
    pub fn load_network(
        &mut self,
        uri: &str,
        credentials: Option<crate::services::network::NetworkCredentials>,
        sender: relm4::AsyncComponentSender<Self>,
    ) {
        let uri_str = uri.to_string();
        let expand_labels = self.config.ui.expand_labels;

        relm4::spawn_blocking(move || {
            match crate::services::network::list_network_entries(&uri_str, credentials.as_ref()) {
                Ok(entries) => {
                    let contexts =
                        crate::services::network::entries_to_load_contexts(&entries, expand_labels);
                    sender.input(AppMsg::NetworkLoaded {
                        uri: uri_str,
                        contexts,
                    });
                }
                Err(crate::services::network::NetworkError::CredentialsRequired {
                    message,
                    flags,
                }) => {
                    sender.input(AppMsg::PromptNetworkCredentials {
                        uri: uri_str,
                        message,
                        flags,
                        auth_failed: false,
                    });
                }
                Err(crate::services::network::NetworkError::AuthFailed) => {
                    sender.input(AppMsg::PromptNetworkCredentials {
                        uri: uri_str,
                        message: crate::i18n::tr(
                            "Authentication failed. Please check your credentials.",
                        ),
                        flags: crate::services::network::NetworkAuthFlags::USERNAME
                            | crate::services::network::NetworkAuthFlags::PASSWORD,
                        auth_failed: true,
                    });
                }
                Err(e) => {
                    sender.input(AppMsg::ShowToast(e.to_string()));
                }
            }
        });
    }

    /// Populates the file grid with the immediate children of `prefix` inside the
    /// archive located at `archive_path`.
    ///
    /// The virtual current path is set to an `archive://` URI so that breadcrumb
    /// rendering, history management, and the back-button all work identically to
    /// real directory navigation. No files are extracted to disk.
    ///
    /// # Arguments
    /// * `archive_path` - Real on-disk path of the archive file.
    /// * `prefix`       - Inner path component being listed (`""` = root level).
    /// * `sender`       - Component handle for dispatching lifecycle messages.
    pub fn load_archive(
        &mut self,
        archive_path: PathBuf,
        prefix: String,
        mut password: Option<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        if let Some(old_mon) = self.directory_monitor.take() {
            old_mon.cancel();
        }
        self.files.clear();
        self.is_loading = true;
        // Bump the session counter and capture the resulting ID so the
        // spawned closure can stamp the message it will later dispatch.
        let session_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.pending_thumbnails.clear();

        self.current_path = archive::build_archive_uri(&archive_path, &prefix);

        if password.is_none() {
            password = self.cached_archive_password.clone();
        }

        let archive_path_c = archive_path.clone();
        let prefix_c = prefix.clone();
        let password_c = password.clone();
        let sender_c = sender.clone();

        relm4::spawn_blocking(move || {
            let result =
                archive::list_archive_entries(&archive_path_c, &prefix_c, password_c.as_deref());

            sender_c.input(AppMsg::ArchiveLoaded {
                archive_path: archive_path_c,
                prefix: prefix_c,
                password: password_c,
                load_id: session_id,
                result,
            });
        });
    }

    pub fn handle_archive_loaded(
        &mut self,
        archive_path: PathBuf,
        prefix: String,
        password: Option<String>,
        load_id: u64,
        result: Result<Vec<archive::ArchiveEntry>, archive::ArchiveError>,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.is_loading = false;

        // Discard results from superseded navigation sessions.
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }

        let current_session = load_id;
        let expand_labels = self.config.ui.expand_labels;
        let sort_strategy = self.sort_by;
        let sort_ascending = self.sort_ascending;
        let folders_first = self.config.ui.folders_first;
        let extension_globset = self.extension_globset.clone();

        match result {
            Err(archive::ArchiveError::PasswordRequired) => {
                self.cached_archive_password = None;
                sender.input(AppMsg::PromptArchivePassword {
                    archive_path,
                    prefix,
                    wrong_password: false,
                });
            }
            Err(archive::ArchiveError::WrongPassword) => {
                self.cached_archive_password = None;
                sender.input(AppMsg::PromptArchivePassword {
                    archive_path,
                    prefix,
                    wrong_password: true,
                });
            }
            Err(archive::ArchiveError::Other(e)) => {
                sender.input(AppMsg::ShowToast(e));
            }
            Ok(entries) => {
                if password.is_some() {
                    self.cached_archive_password = password;
                }

                let mut items =
                    archive::entries_to_load_contexts(&entries, &archive_path, expand_labels);

                if let Some(ref gs) = extension_globset {
                    items.retain(|item| {
                        if item.is_dir {
                            return true;
                        }
                        gs.is_match(&item.sort_name)
                    });
                }

                // Sort entries
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
                            SortBy::Size => a.size.cmp(&b.size),
                            SortBy::Date => a.mtime.cmp(&b.mtime),
                            SortBy::Type => a.sort_ext.cmp(&b.sort_ext),
                        };

                        let tie = if primary_order == std::cmp::Ordering::Equal {
                            a.sort_name.cmp(&b.sort_name)
                        } else {
                            primary_order
                        };

                        if sort_ascending {
                            tie
                        } else {
                            tie.reverse()
                        }
                    });
                });

                if self.is_list_mode {
                    self.files.view.set_min_columns(1);
                    self.files.view.set_max_columns(1);
                } else {
                    self.files.view.set_min_columns(1);
                    self.files.view.set_max_columns(20);
                }

                let mut media_tasks: Vec<(u32, PathBuf)> = Vec::new();
                for (grid_idx, item) in (self.files.len()..).zip(items) {
                    let icon = utils::get_icon_for_path(&item.target_path, item.is_dir);

                    // Collect visual media files for thumbnail generation
                    if !item.is_dir {
                        let (is_img, is_vid) = is_visual_media_by_ext(&item.target_path);
                        if is_img || is_vid {
                            media_tasks.push((grid_idx, item.target_path.clone()));
                        }
                    }

                    self.files.append(FileItem {
                        name: item.display_name.clone(),
                        icon,
                        thumbnail: None,
                        is_dir: item.is_dir,
                        path: item.target_path,
                        icon_size: if self.is_list_mode {
                            self.current_list_icon_size
                        } else {
                            self.current_icon_size
                        },
                        size: item.size,
                        mtime: item.mtime,
                        is_editing: false,
                        is_foreign_owner: false,
                        expand_labels: item.expand_labels,
                        is_list_mode: self.is_list_mode,
                        is_custom_icon: false,
                        active_path: Rc::new(RefCell::new(None)),
                        grid_idx,
                        max_width_chars: self.config.ui.max_width_chars,
                        grid_spacing: self.config.ui.grid_spacing,
                    });
                }

                self.update_breadcrumbs();

                if !self.config.ui.lazy_thumbnails {
                    self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
                } else {
                    sender.input(AppMsg::CheckVisibleThumbnails);
                }
            }
        }
    }
    /// Populates the file grid with entries from the GTK recent-files registry.
    ///
    /// Parses `~/.local/share/recently-used.xbel` with the standard XML reader.
    /// Each `<bookmark href="file://...">` element carries a `visited` timestamp
    /// in its `<info><metadata><mime-type>` subtree, which is used to sort newest-first.
    /// Entries whose backing file no longer exists on disk are silently skipped.
    ///
    /// # Arguments
    /// * `sender` - Component handle used to dispatch lifecycle updates.
    pub fn load_recents(&mut self, sender: &AsyncComponentSender<Self>) {
        self.is_loading = true;
        if let Some(old_mon) = self.directory_monitor.take() {
            old_mon.cancel();
        }
        self.files.clear();
        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.pending_thumbnails.clear();
        self.current_path = std::path::PathBuf::from(crate::ui::constants::RECENT_URI);

        let xbel_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(".local/share"))
            .join("recently-used.xbel");

        let xml = match std::fs::read_to_string(&xbel_path) {
            Ok(s) => s,
            Err(_) => {
                self.update_breadcrumbs();
                self.is_loading = false;
                return;
            }
        };

        // Extract (visited_rfc3339, href) from each <bookmark> element.
        // The XBEL format places `visited` as an attribute on the <bookmark> tag itself:
        //   <bookmark href="file:///path/to/file" added="..." modified="..." visited="...">
        let mut entries: Vec<(String, String)> = xml
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if !line.starts_with("<bookmark ") {
                    return None;
                }
                let href = Self::xbel_attr(line, "href")?;
                if !href.starts_with("file://") {
                    return None;
                }
                // Use `modified` as the recency signal, `visited` is often absent.
                let ts = Self::xbel_attr(line, "modified")
                    .or_else(|| Self::xbel_attr(line, "added"))
                    .unwrap_or_default();
                Some((ts, href))
            })
            .collect();

        // RFC 3339 timestamps sort lexicographically, so string comparison is correct.
        entries.sort_by(|a, b| b.0.cmp(&a.0));
        entries.truncate(crate::ui::constants::MAX_RECENT_ITEMS);

        if self.is_list_mode {
            self.files.view.set_min_columns(1);
            self.files.view.set_max_columns(1);
        } else {
            self.files.view.set_min_columns(1);
            self.files.view.set_max_columns(20);
        }

        let mut media_tasks: Vec<(u32, PathBuf)> = Vec::new();
        let extension_globset = self.extension_globset.clone();

        for (grid_idx, (_ts, href)) in entries.into_iter().enumerate() {
            let gfile = gio::File::for_uri(&href);
            let Some(path) = gfile.path() else { continue };
            if !path.exists() {
                continue;
            }

            let display_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| href.clone());

            let is_dir = path.is_dir();

            // Session-scoped glob filter for recents view.
            if !is_dir {
                if let Some(ref gs) = extension_globset {
                    if !gs.is_match(display_name.to_lowercase()) {
                        continue;
                    }
                }
            }

            let icon = utils::get_icon_for_path(&path, is_dir);

            let (is_img, is_vid) = is_visual_media_by_ext(&path);
            if is_img || is_vid {
                media_tasks.push((self.files.len(), path.clone()));
            }

            self.files.append(crate::ui::FileItem {
                name: display_name,
                icon,
                thumbnail: None,
                is_dir,
                path,
                icon_size: if self.is_list_mode {
                    self.current_list_icon_size
                } else {
                    self.current_icon_size
                },
                size: 0,
                mtime: 0,
                is_editing: false,
                is_foreign_owner: false,
                expand_labels: self.config.ui.expand_labels,
                is_list_mode: self.is_list_mode,
                is_custom_icon: false,
                active_path: Rc::new(RefCell::new(None)),
                grid_idx: grid_idx as u32,
                max_width_chars: self.config.ui.max_width_chars,
                grid_spacing: self.config.ui.grid_spacing,
            });
        }

        self.update_breadcrumbs();

        if !self.config.ui.lazy_thumbnails {
            self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
        } else {
            sender.input(AppMsg::CheckVisibleThumbnails);
        }
        self.is_loading = false;
    }

    /// Extracts the value of a named XML attribute from a single-line tag string.
    ///
    /// Matches the pattern `name="value"` or `name='value'` and returns the value.
    /// Only intended for the simple flat attributes on XBEL `<bookmark>` elements.
    ///
    /// # Arguments
    /// * `tag` - A single line of XML containing the attribute.
    /// * `name` - The attribute name to search for.
    fn xbel_attr(tag: &str, name: &str) -> Option<String> {
        // XBEL files produced by GTK always use double-quoted attributes.
        let needle = format!("{}=\"", name);
        let start = tag.find(&needle)? + needle.len();
        let rest = &tag[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_owned())
    }

    /// construct and append a slice of FileLoadContext items directly to self.files.
    pub fn append_context_batch(
        &mut self,
        items: Vec<FileLoadContext>,
        load_id: u64,
        _is_cached: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        let max_width_chars = self.config.ui.max_width_chars;
        let grid_spacing = self.config.ui.grid_spacing;
        let is_list_mode = self.is_list_mode;
        let list_icon_size = self.current_list_icon_size;
        let grid_icon_size = self.current_icon_size;
        let config_file_icons = &self.config.ui.file_icons;
        let config_folder_icons = &self.config.ui.folder_icons;
        let cached_thumbs = self
            .folder_cache
            .get(&self.current_path)
            .map(|c| &c.thumbnails);

        let start_idx = self.files.len();
        let mut chunk_media_tasks: Vec<(u32, PathBuf)> = Vec::new();

        for (offset, item) in items.into_iter().enumerate() {
            let grid_idx = start_idx + offset as u32;

            let custom_icon = config_file_icons
                .get(&item.target_path.to_string_lossy().to_string())
                .cloned()
                .or_else(|| {
                    if item.is_dir {
                        config_folder_icons
                            .get(&item.target_path.to_string_lossy().to_string())
                            .cloned()
                    } else {
                        None
                    }
                })
                .or(item.custom_icon);

            let icon = if let Some(ref custom) = custom_icon {
                gtk::gio::Icon::for_string(custom)
                    .unwrap_or_else(|_| utils::get_icon_for_path(&item.target_path, item.is_dir))
            } else {
                utils::get_icon_for_path(&item.target_path, item.is_dir)
            };

            let thumbnail = cached_thumbs
                .and_then(|m| m.get(&item.target_path))
                .cloned();

            // Collect visual media files that don't have a thumbnail yet
            if !item.is_dir && thumbnail.is_none() {
                let (is_img, is_vid) = is_visual_media_by_ext(&item.target_path);
                if is_img || is_vid {
                    let source = custom_icon
                        .as_ref()
                        .map(PathBuf::from)
                        .or_else(|| item.thumbnail_path.clone())
                        .unwrap_or_else(|| item.target_path.clone());
                    chunk_media_tasks.push((grid_idx, source));
                }
            }

            let file_item = FileItem {
                name: item.display_name,
                icon,
                thumbnail,
                is_dir: item.is_dir,
                path: item.target_path,
                icon_size: if is_list_mode {
                    list_icon_size
                } else {
                    grid_icon_size
                },
                size: item.size,
                mtime: item.mtime,
                is_editing: false,
                is_foreign_owner: item.is_foreign_owner,
                expand_labels: item.expand_labels,
                is_list_mode,
                is_custom_icon: custom_icon.is_some(),
                active_path: Rc::new(RefCell::new(None)),
                grid_idx,
                max_width_chars,
                grid_spacing,
            };

            self.files.append(file_item);
        }

        // Fire thumbnail generation for any items still missing thumbnails
        if !chunk_media_tasks.is_empty() {
            if !self.config.ui.lazy_thumbnails {
                self.spawn_thumbnail_loader(chunk_media_tasks, load_id, sender.clone());
            } else {
                sender.input(AppMsg::CheckVisibleThumbnails);
            }
        }
    }

    /// Streams background folder results into the grid in batches.
    /// Discards stale sessions by checking `load_id`.
    pub fn handle_folder_loaded(
        &mut self,
        path: PathBuf,
        load_id: u64,
        items: Vec<FileLoadContext>,
        media_tasks: Vec<(u32, PathBuf)>,
        sender: &AsyncComponentSender<Self>,
    ) {
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }

        let is_cached = self.folder_cache.contains_key(&path);

        if !path.to_string_lossy().starts_with("trash://")
            && !path
                .to_string_lossy()
                .starts_with(crate::services::archive::ARCHIVE_URI)
            && self.filter.is_empty()
            && self.extension_globset.is_none()
            && !is_cached
        {
            let cache_cap = self.config.ui.folder_cache_capacity;
            if cache_cap > 0 && !is_cached {
                if self.folder_cache.len() >= cache_cap {
                    if let Some(oldest) = self
                        .folder_cache
                        .iter()
                        .min_by_key(|(_, v)| v.last_visited)
                        .map(|(k, _)| k.clone())
                    {
                        self.folder_cache.remove(&oldest);
                        unsafe {
                            libc::malloc_trim(0);
                        }
                    }
                }
            }

            self.folder_cache.insert(
                path.clone(),
                crate::model::CachedFolder {
                    items: items.clone(),
                    media_tasks,
                    thumbnails: std::collections::HashMap::new(),
                    last_visited: std::time::Instant::now(),
                },
            );
        }

        // CLEAR the grid completely so Relm4 drops all old FileItems
        // and cleans up widget associations, qdata, and MultiSelection state.
        // WARNING: this is necessary for folder cache, if no clear here, it will start mixing files
        // from different folders and cause all sorts of problems.
        self.files.clear();

        if self.is_list_mode {
            self.files.view.set_min_columns(1);
            self.files.view.set_max_columns(1);
        } else {
            self.files.view.set_min_columns(1);
            self.files.view.set_max_columns(20);
        }

        self.current_path = path;
        self.update_breadcrumbs();

        let batch_size = self.config.ui.loader_batch_size.max(10);

        if items.len() <= batch_size {
            self.append_context_batch(items, load_id, is_cached, sender);
            self.is_loading = false;
            unsafe {
                libc::malloc_trim(0);
            }
        } else {
            let mut remaining = items;
            let first_batch: Vec<FileLoadContext> = remaining.drain(..batch_size).collect();
            // Append and immediately trigger thumbnails for the initial visible batch
            self.append_context_batch(first_batch, load_id, is_cached, sender);

            let session_arc = self.load_id.clone();
            let sender_clone = sender.clone();
            let mut chunks: Option<Vec<Vec<FileLoadContext>>> = {
                let mut v: Vec<Vec<FileLoadContext>> =
                    remaining.chunks(batch_size).map(|c| c.to_vec()).collect();
                v.reverse();
                Some(v)
            };

            glib::idle_add_local(move || {
                if session_arc.load(Ordering::SeqCst) != load_id {
                    chunks.take(); // free all pending item data immediately
                    return glib::ControlFlow::Break;
                }

                if let Some(chunk) = chunks.as_mut().and_then(|v| v.pop()) {
                    sender_clone.input(AppMsg::FolderLoadedChunk {
                        load_id,
                        chunk,
                        is_cached,
                    });

                    if chunks.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                        sender_clone.input(AppMsg::FolderLoadedFinish { load_id });
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                } else {
                    glib::ControlFlow::Break
                }
            });
        }
    }
}
