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

            let mut items: Vec<FileLoadContext> = raw_data
                .into_par_iter()
                .filter_map(|(name, display_name, is_dir, size, mtime, uid)| {
                    if !show_hidden && name.starts_with('.') {
                        return None;
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
                        config_folder_icons.get(&path_key).cloned()
                    } else {
                        None
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

                self.files.append(FileItem {
                    name: item.display_name.clone(),
                    icon,
                    thumbnail: None,
                    is_dir: item.is_dir,
                    path: item.target_path,
                    icon_size: self.current_icon_size,
                    size: item.size,
                    is_editing: false,
                    is_foreign_owner: item.is_foreign_owner,
                    expand_labels: item.expand_labels,
                    is_list_mode: self.is_list_mode,
                    is_custom_icon: item.custom_icon.is_some(),
                    active_path: Rc::new(RefCell::new(None)),
                });

                if let Some(abs_path) = item.thumbnail_path {
                    media_tasks.push((item.display_name, abs_path));
                }
            }

            self.current_path = path;

            self.spawn_thumbnail_loader(media_tasks, current_session, sender.clone());
        }
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
        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;

        self.current_path = archive::build_archive_uri(&archive_path, &prefix);

        if password.is_none() {
            password = self.cached_archive_password.clone();
        }

        let expand_labels = self.config.ui.expand_labels;
        let sort_strategy = self.sort_by;
        let sort_ascending = self.sort_ascending;
        let folders_first = self.config.ui.folders_first;

        match archive::list_archive_entries(&archive_path, &prefix, password.as_deref()) {
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

                // Pass the extracted media tasks into your existing loader!
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SortBy;
    use std::path::Path;
    use std::path::PathBuf;

    fn mock_ctx(name: &str, is_dir: bool, size: u64, mtime: i64) -> FileLoadContext {
        FileLoadContext {
            display_name: name.to_string(),
            sort_name: name.to_lowercase(),
            target_path: PathBuf::from(name),
            size,
            mtime,
            is_dir,
            thumbnail_path: None,
            is_foreign_owner: false,
            expand_labels: false,
            custom_icon: None,
        }
    }

    #[test]
    fn test_is_visual_media_by_ext() {
        let img_exts = [
            "jpg", "jpeg", "png", "gif", "webp", "avif", "heic", "heif", "bmp", "tiff", "tif",
            "jxl", "svg", "pdf", "ttf", "otf", "woff", "woff2", "ttc",
        ];
        for ext in img_exts {
            let file_name = format!("file.{}", ext);
            let path = Path::new(&file_name);
            let (is_img, is_vid) = is_visual_media_by_ext(path);
            assert!(is_img, "image extension .{} should be true", ext);
            assert!(!is_vid, "image extension .{} should not be video", ext);
        }

        let vid_exts = [
            "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv", "m4v", "mpg", "mpeg", "ts", "ogv",
        ];
        for ext in vid_exts {
            let file_name = format!("file.{}", ext);
            let path = Path::new(&file_name);
            let (is_img, is_vid) = is_visual_media_by_ext(path);
            assert!(!is_img, "video extension .{} should not be image", ext);
            assert!(is_vid, "video extension .{} should be true", ext);
        }

        let path = Path::new("file.txt");
        let (is_img, _) = is_visual_media_by_ext(path);
        assert!(!is_img);

        let path = Path::new("file.JPG");
        let (is_img, _) = is_visual_media_by_ext(path);
        assert!(is_img);
    }

    #[test]
    fn test_xbel_attr() {
        let line =
            r#"<bookmark href="file:///home/user/file.txt" modified="2025-01-01T12:00:00Z"/>"#;
        assert_eq!(
            FluxApp::xbel_attr(line, "href"),
            Some("file:///home/user/file.txt".to_string())
        );
        assert_eq!(
            FluxApp::xbel_attr(line, "modified"),
            Some("2025-01-01T12:00:00Z".to_string())
        );

        let line = r#"<bookmark href="file:///home/user/file.txt"/>"#;
        assert_eq!(FluxApp::xbel_attr(line, "added"), None);
    }

    fn sort_items(items: &mut [FileLoadContext], by: SortBy, folders_first: bool, ascending: bool) {
        use rayon::prelude::*;
        items.par_sort_unstable_by(|a, b| {
            if a.is_dir != b.is_dir {
                return if folders_first {
                    b.is_dir.cmp(&a.is_dir)
                } else {
                    a.is_dir.cmp(&b.is_dir)
                };
            }
            let primary = match by {
                SortBy::Name => a.sort_name.cmp(&b.sort_name),
                SortBy::Size => a.size.cmp(&b.size),
                SortBy::Date => a.mtime.cmp(&b.mtime),
                SortBy::Type => {
                    let ext_a = Path::new(&a.display_name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let ext_b = Path::new(&b.display_name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    ext_a.cmp(&ext_b)
                }
            };
            let tie = if primary == std::cmp::Ordering::Equal {
                a.sort_name.cmp(&b.sort_name)
            } else {
                primary
            };
            if ascending {
                tie
            } else {
                tie.reverse()
            }
        });
    }

    #[test]
    fn test_sort_by_name() {
        let mut items = vec![
            mock_ctx("Zebra", false, 0, 0),
            mock_ctx("Apple", false, 0, 0),
            mock_ctx("Banana", false, 0, 0),
        ];
        sort_items(&mut items, SortBy::Name, true, true);
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
        sort_items(&mut items, SortBy::Size, true, false);
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
        sort_items(&mut items, SortBy::Date, true, false);
        assert_eq!(items[0].display_name, "New");
        assert_eq!(items[1].display_name, "Mid");
        assert_eq!(items[2].display_name, "Old");
    }

    #[test]
    fn test_sort_by_type() {
        let mut items = vec![
            mock_ctx("file.a", false, 0, 0),
            mock_ctx("file.b", false, 0, 0),
            mock_ctx("file.A", false, 0, 0),
        ];
        sort_items(&mut items, SortBy::Type, true, true);
        assert_eq!(items[0].display_name, "file.a");
        assert_eq!(items[1].display_name, "file.A");
        assert_eq!(items[2].display_name, "file.b");
    }

    #[test]
    fn test_folders_first_sorting() {
        let mut items = vec![
            mock_ctx("file.txt", false, 0, 0),
            mock_ctx("DirA", true, 0, 0),
            mock_ctx("file2.txt", false, 0, 0),
            mock_ctx("DirB", true, 0, 0),
        ];
        sort_items(&mut items, SortBy::Name, true, true);
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
        sort_items(&mut items, SortBy::Name, false, true);
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
    fn test_filter_by_name() {
        let items = vec![
            mock_ctx("file1.txt", false, 0, 0),
            mock_ctx("file2.txt", false, 0, 0),
            mock_ctx("document.pdf", false, 0, 0),
        ];
        let query = "file";
        let filtered: Vec<_> = items
            .into_iter()
            .filter(|item| {
                item.display_name
                    .to_lowercase()
                    .contains(&query.to_lowercase())
            })
            .collect();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|i| i.display_name == "file1.txt"));
        assert!(filtered.iter().any(|i| i.display_name == "file2.txt"));
    }

    #[test]
    fn test_foreign_owner_detection() {
        let current_uid = unsafe { libc::getuid() };
        let foreign_uid = current_uid + 1;

        let mut ctx = FileLoadContext {
            display_name: "file".to_string(),
            sort_name: "file".to_string(),
            target_path: PathBuf::from("/test/file"),
            size: 100,
            mtime: 0,
            is_dir: false,
            thumbnail_path: None,
            is_foreign_owner: false,
            expand_labels: false,
            custom_icon: None,
        };

        let is_trash = false;
        let uid = foreign_uid;
        ctx.is_foreign_owner = !is_trash && uid != current_uid;
        assert!(ctx.is_foreign_owner);

        let uid = current_uid;
        ctx.is_foreign_owner = !is_trash && uid != current_uid;
        assert!(!ctx.is_foreign_owner);

        let is_trash = true;
        ctx.is_foreign_owner = !is_trash && uid != current_uid;
        assert!(!ctx.is_foreign_owner);
    }

    #[test]
    fn test_custom_icon_flag() {
        let ctx_with = FileLoadContext {
            display_name: "dir".to_string(),
            sort_name: "dir".to_string(),
            target_path: PathBuf::from("/test/dir"),
            size: 0,
            mtime: 0,
            is_dir: true,
            thumbnail_path: None,
            is_foreign_owner: false,
            expand_labels: false,
            custom_icon: Some("custom".to_string()),
        };
        let ctx_without = FileLoadContext {
            custom_icon: None,
            ..ctx_with
        };

        assert!(ctx_with.custom_icon.is_some());
        assert!(!ctx_without.custom_icon.is_some());
    }
}
