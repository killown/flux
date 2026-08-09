use crate::model::{AppMsg, FileLoadContext, FluxApp, SortBy};
use crate::services::archive;
use crate::ui::FileItem;
use crate::utils;
use adw::prelude::*;
use gtk::gio;
use rayon::prelude::*;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;

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
        self.is_loading = true;
        let path_str = path.to_string_lossy().to_string();

        // Network URIs must go through load_network, gio::File::for_path and
        // std::fs have no URI awareness and will silently produce an empty listing.
        if crate::services::network::is_network_uri(&path) {
            self.current_path = path.clone();
            self.load_network(&path_str, None, sender.clone());
            return;
        }

        self.directory_monitor = None;
        let mut folders_first = self.config.ui.folders_first;

        // Load persistent folder state from SQLite before scanning
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
            "standard::name,standard::display-name,standard::type,standard::size,time::modified,unix::uid";

        if let Ok(enumerator) = root.enumerate_children(
            attributes,
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        ) {
            let raw_data: Vec<(String, String, bool, u64, i64, u32)> = enumerator
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
                        info.attribute_uint32("unix::uid"),
                    )
                })
                .collect();

            let show_hidden = self.show_hidden;
            let sort_strategy = self.sort_by;
            let sort_ascending = self.sort_ascending;
            let is_trash = path_str.starts_with("trash://");

            let current_uid: u32 = unsafe { libc::getuid() };

            let config_folder_icons = self.config.ui.folder_icons.clone();
            // Clone once so the Rayon closure can own it without borrowing `self`.
            let config_file_icons = self.config.ui.file_icons.clone();
            let extension_filter = self.extension_filter.clone();

            let mut items: Vec<FileLoadContext> = raw_data
                .into_par_iter()
                .filter_map(|(name, display_name, is_dir, size, mtime, uid)| {
                    if !show_hidden && name.starts_with('.') {
                        return None;
                    }

                    // Session-scoped glob filter, directories always pass through.
                    if !is_dir {
                        if let Some(ref patterns) = extension_filter {
                            let name_lc = display_name.to_lowercase();
                            let matched = patterns
                                .iter()
                                .any(|p| crate::utils::glob::glob_match(p, &name_lc));
                            if !matched {
                                return None;
                            }
                        }
                    }

                    let target_path = if is_trash {
                        PathBuf::from(root.child(&name).uri())
                    } else {
                        path.join(&name)
                    };

                    let mut thumbnail_path = None;
                    if !is_dir {
                        let (is_img, is_vid) = is_visual_media_by_ext(&target_path);
                        if is_img || is_vid {
                            thumbnail_path = Some(target_path.clone());
                        }
                    }

                    let custom_icon = if is_dir {
                        let path_key = target_path.to_string_lossy().to_string();
                        // Image-based override takes priority over the GTK icon name picker.
                        config_file_icons
                            .get(&path_key)
                            .map(|_| path_key.clone())
                            .or_else(|| config_folder_icons.get(&path_key).cloned())
                    } else {
                        let path_key = target_path.to_string_lossy().to_string();
                        // A custom image path stored in `file_icons` takes priority over the system icon.
                        // It is rendered as a thumbnail texture via the thumbnail pipeline.
                        config_file_icons.get(&path_key).map(|_| path_key)
                    };

                    Some(FileLoadContext {
                        sort_name: display_name.to_lowercase(),
                        display_name,
                        target_path,
                        size,
                        mtime,
                        is_dir,
                        thumbnail_path,
                        is_foreign_owner: !is_trash && uid != current_uid,
                        expand_labels: self.config.ui.expand_labels,
                        custom_icon,
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

                let primary_order = match sort_strategy {
                    SortBy::Name => a
                        .display_name
                        .to_lowercase()
                        .cmp(&b.display_name.to_lowercase()),
                    SortBy::Size => a.size.cmp(&b.size),
                    SortBy::Date => a.mtime.cmp(&b.mtime),
                    SortBy::Type => {
                        let ext_a = std::path::Path::new(&a.display_name)
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();

                        let ext_b = std::path::Path::new(&b.display_name)
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();

                        ext_a.cmp(&ext_b)
                    }
                };

                let tie_breaker = if primary_order == std::cmp::Ordering::Equal {
                    a.display_name
                        .to_lowercase()
                        .cmp(&b.display_name.to_lowercase())
                } else {
                    primary_order
                };

                if sort_ascending {
                    tie_breaker
                } else {
                    tie_breaker.reverse()
                }
            });

            let mut media_tasks = Vec::new();
            for item in items {
                let icon = if let Some(ref custom) = item.custom_icon {
                    gtk::gio::Icon::for_string(custom).unwrap_or_else(|_| {
                        utils::get_icon_for_path(&item.target_path, item.is_dir)
                    })
                } else {
                    utils::get_icon_for_path(&item.target_path, item.is_dir)
                };

                // Resolve thumbnail source before `item` is partially moved into FileItem.
                // Covers: visual-media files, file-level custom image overrides, and
                // directory-level custom image overrides (bypasses GTK icon name limit).
                let thumb_source = item.thumbnail_path.clone().or_else(|| {
                    let key = item.target_path.to_string_lossy().to_string();
                    config_file_icons.get(&key).map(PathBuf::from)
                });

                self.files.append(FileItem {
                    name: item.display_name.clone(),
                    icon,
                    thumbnail: None,
                    is_dir: item.is_dir,
                    path: item.target_path.clone(),
                    icon_size: self.current_icon_size,
                    size: item.size,
                    is_editing: false,
                    is_foreign_owner: item.is_foreign_owner,
                    expand_labels: item.expand_labels,
                    is_list_mode: self.is_list_mode,
                    is_custom_icon: item.custom_icon.is_some(),
                    active_path: Rc::new(RefCell::new(None)),
                });

                if let Some(abs_path) = thumb_source {
                    media_tasks.push((item.display_name, abs_path));
                }
            }

            self.current_path = path;

            self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
        }

        self.is_loading = false;
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
        self.directory_monitor = None;
        self.files.clear();
        self.is_loading = true;
        // Bump the session counter and capture the resulting ID so the
        // spawned closure can stamp the message it will later dispatch.
        let session_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;

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

                let extension_filter = self.extension_filter.clone();
                if let Some(ref patterns) = extension_filter {
                    items.retain(|item| {
                        if item.is_dir {
                            return true;
                        }
                        let name_lc = item.display_name.to_lowercase();
                        patterns
                            .iter()
                            .any(|p| crate::utils::glob::glob_match(p, &name_lc))
                    });
                }

                // Sort entries
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
                        SortBy::Type => {
                            let ext_a = std::path::Path::new(&a.display_name)
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let ext_b = std::path::Path::new(&b.display_name)
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            ext_a.cmp(&ext_b)
                        }
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

                let mut media_tasks = Vec::new();

                for item in items {
                    let icon = utils::get_icon_for_path(&item.target_path, item.is_dir);

                    // Collect visual media files for thumbnail generation
                    if !item.is_dir {
                        let (is_img, is_vid) = is_visual_media_by_ext(&item.target_path);
                        if is_img || is_vid {
                            media_tasks.push((item.display_name.clone(), item.target_path.clone()));
                        }
                    }

                    self.files.append(FileItem {
                        name: item.display_name.clone(),
                        icon,
                        thumbnail: None,
                        is_dir: item.is_dir,
                        path: item.target_path,
                        icon_size: self.current_icon_size,
                        size: item.size,
                        is_editing: false,
                        is_foreign_owner: false,
                        expand_labels: item.expand_labels,
                        is_list_mode: self.is_list_mode,
                        is_custom_icon: false,
                        active_path: Rc::new(RefCell::new(None)),
                    });
                }

                self.update_breadcrumbs();
                self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
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
        self.directory_monitor = None;
        self.files.clear();
        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.current_path = std::path::PathBuf::from(crate::ui::constants::RECENT_URI);

        let xbel_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(".local/share"))
            .join("recently-used.xbel");

        let xml = match std::fs::read_to_string(&xbel_path) {
            Ok(s) => s,
            Err(_) => {
                self.update_breadcrumbs();
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

        let mut media_tasks = Vec::new();
        for (_ts, href) in entries {
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
                if let Some(ref patterns) = self.extension_filter {
                    let name_lc = display_name.to_lowercase();
                    let matched = patterns
                        .iter()
                        .any(|p| crate::utils::glob::glob_match(p, &name_lc));
                    if !matched {
                        continue;
                    }
                }
            }

            let icon = utils::get_icon_for_path(&path, is_dir);

            let (is_img, is_vid) = is_visual_media_by_ext(&path);
            if is_img || is_vid {
                media_tasks.push((display_name.clone(), path.clone()));
            }

            self.files.append(crate::ui::FileItem {
                name: display_name,
                icon,
                thumbnail: None,
                is_dir,
                path,
                icon_size: self.current_icon_size,
                size: 0,
                is_editing: false,
                is_foreign_owner: false,
                expand_labels: self.config.ui.expand_labels,
                is_list_mode: self.is_list_mode,
                is_custom_icon: false,
                active_path: Rc::new(RefCell::new(None)),
            });
        }

        self.update_breadcrumbs();
        self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
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
}
