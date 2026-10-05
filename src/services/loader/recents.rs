use super::media::is_visual_media_by_ext;
use crate::model::{AppMsg, FluxApp};
use crate::ui::FileItem;
use crate::utils;
use crate::utils::media::is_audio_file;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
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
        self.thumbnail_manager.clear_and_cancel_all();

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

            let icon = utils::icon::get_icon_for_path(&path, is_dir);

            let (is_img, is_vid) = is_visual_media_by_ext(&path);
            let is_audio = is_audio_file(&path);
            if is_img || is_vid || is_audio {
                media_tasks.push((self.files.len(), path.clone()));
            }

            let is_empty = if is_dir && self.config.ui.show_empty_dir_emblem {
                FluxApp::is_dir_empty(&path)
            } else {
                false
            };

            let real_path = Self::expand_path(&path.to_string_lossy());
            let (is_symlink, is_broken_symlink) = if real_path.is_symlink() {
                (true, std::fs::metadata(&real_path).is_err())
            } else {
                (false, false)
            };

            let display_label = crate::utils::helpers::format_display_label(
                &display_name,
                is_dir,
                &self.config.ui.hidden_extensions,
            );

            self.files.append(
                FileItem::builder(display_name, path, icon)
                    .display_label(display_label)
                    .is_dir(is_dir)
                    .is_empty(is_empty)
                    .expand_labels(self.config.ui.expand_labels)
                    .icon_size(if self.is_list_mode {
                        self.current_list_icon_size
                    } else {
                        self.current_icon_size
                    })
                    .is_list_mode(self.is_list_mode)
                    .grid_idx(grid_idx as u32)
                    .max_width_chars(self.config.ui.max_width_chars)
                    .grid_spacing(self.config.ui.grid_spacing)
                    .is_symlink(is_symlink)
                    .is_broken_symlink(is_broken_symlink)
                    .show_symlink_emblem(self.config.ui.show_symlink_emblem)
                    .scale_font_with_icons(self.config.ui.scale_font_with_icons)
                    .default_icon_size(self.config.ui.default_icon_size)
                    .show_empty_dir_emblem(self.config.ui.show_empty_dir_emblem)
                    .disable_drag_and_drop(self.config.ui.disable_drag_and_drop)
                    .build(),
            );
        }

        self.update_breadcrumbs();

        if !self.config.ui.lazy_thumbnails {
            self.spawn_thumbnail_loader(
                media_tasks,
                current_session,
                self.active_tab_index,
                sender.clone(),
            );
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
}
