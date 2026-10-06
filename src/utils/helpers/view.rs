//! Sort status, list mode sync, breadcrumbs and search result batches.

use crate::model::{FluxApp, PathSegment, SortBy};
use crate::ui::constants;
use crate::utils;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Returns the display-friendly string for the current sorting state.
    pub fn sort_status(&self) -> String {
        let arrow = if self.sort_ascending { " ↑" } else { " ↓" };
        format!(
            "{}{}",
            match self.sort_by {
                SortBy::Name => crate::i18n::tr("Name"),
                SortBy::Date => crate::i18n::tr("Date"),
                SortBy::Size => crate::i18n::tr("Size"),
                SortBy::Type => crate::i18n::tr("Type"),
            },
            arrow
        )
    }

    /// Returns the `GVariant` string state for the stateful `app.sort-direction` radio action.
    pub fn sort_direction_state(&self) -> gtk::glib::Variant {
        gtk::glib::Variant::from(if self.sort_ascending { "asc" } else { "desc" })
    }

    /// Propagates the current `is_list_mode` flag to every `FileItem` in the grid.
    ///
    /// Must be called after toggling list mode so `bind()` re-renders each item
    /// with the correct orientation and icon size.
    pub fn sync_list_mode(&mut self) {
        let mode = self.is_list_mode;
        let size = if mode {
            self.current_list_icon_size
        } else {
            self.current_icon_size
        };
        for i in 0..self.files.len() {
            if let Some(wrapper) = self.files.get(i) {
                let mut item = wrapper.borrow().clone();
                // Update both mode and icon size if either changed
                if item.is_list_mode != mode || item.icon_size != size {
                    item.is_list_mode = mode;
                    item.icon_size = size;
                    self.files.remove(i);
                    self.files.insert(i, item);
                }
            }
        }
    }

    /// Updates the collection of path segments for breadcrumb navigation display.
    pub fn update_breadcrumbs(&mut self) {
        let mut guard = self.breadcrumbs.guard();
        guard.clear();

        let path_str = self.current_path.to_string_lossy();

        // Handle virtual archive breadcrumbs differently so they show human-readable names
        if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            if let Some((archive_path, inner)) =
                crate::services::archive::parse_archive_uri(&path_str)
            {
                let archive_name = archive_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Archive".to_string());

                let root_uri = crate::services::archive::build_archive_uri(&archive_path, "");
                guard.push_back(PathSegment {
                    name: archive_name,
                    path: root_uri,
                });

                if !inner.is_empty() {
                    let mut current_inner = PathBuf::new();
                    for component in Path::new(&inner).components() {
                        let name = component.as_os_str().to_string_lossy().to_string();
                        if name.is_empty() {
                            continue;
                        }
                        current_inner.push(&name);
                        let segment_uri = crate::services::archive::build_archive_uri(
                            &archive_path,
                            &current_inner.to_string_lossy(),
                        );
                        guard.push_back(PathSegment {
                            name,
                            path: segment_uri,
                        });
                    }
                }

                if let Some(entry) = self.header_path_entry.upgrade() {
                    entry.set_text(&path_str);
                    entry.set_position(entry.text_length() as i32);
                }
                return;
            }
        }

        // Network URIs (ftp://, smb://, sftp://, etc.) cannot be decomposed via
        // PathBuf::components - the stdlib has no URI awareness and will mangle
        // the scheme double-slash into bogus local paths. Split on '/' manually
        // and reconstruct each breadcrumb as a well-formed URI.
        if crate::services::network::is_network_uri(&self.current_path) {
            let uri = path_str.trim_end_matches('/');

            if let Some((scheme, after_scheme)) = uri.split_once("://") {
                let slash_pos = after_scheme.find('/');
                let authority = &after_scheme[..slash_pos.unwrap_or(after_scheme.len())];

                let root_uri = format!("{}://{}", scheme, authority);
                guard.push_back(PathSegment {
                    name: authority.to_string(),
                    path: PathBuf::from(&root_uri),
                });

                if let Some(path_part) = slash_pos.map(|p| &after_scheme[p + 1..]) {
                    let mut acc = root_uri.clone();
                    for segment in path_part.split('/').filter(|s| !s.is_empty()) {
                        acc.push('/');
                        acc.push_str(segment);
                        guard.push_back(PathSegment {
                            name: segment.to_string(),
                            path: PathBuf::from(&acc),
                        });
                    }
                }
            }

            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&path_str);
                entry.set_position(entry.text_length() as i32);
            }
            return;
        }

        let mut path_acc = PathBuf::from("/");
        let mut segments = Vec::new();

        for component in self.current_path.components() {
            let name = component.as_os_str().to_string_lossy().to_string();
            if name == "/" || name.is_empty() {
                continue;
            }

            path_acc.push(&name);
            segments.push(PathSegment {
                name,
                path: path_acc.clone(),
            });
        }

        let max_visible = constants::MAX_BREADCRUMBS;
        let skip = segments.len().saturating_sub(max_visible);

        for segment in segments.into_iter().skip(skip) {
            guard.push_back(segment);
        }

        if let Some(entry) = self.header_path_entry.upgrade() {
            entry.set_text(&path_str);
            entry.set_position(entry.text_length() as i32);
        }
    }

    /// Appends a batch of extension/glob search results to the grid.
    pub fn handle_extension_search_batch(
        &mut self,
        results: Vec<crate::services::search::ExtensionMatch>,
        session: u64,
    ) {
        if self.load_id.load(Ordering::SeqCst) != session {
            return;
        }

        let active_files = &mut self.tabs[self.active_tab_index].files;
        let start_idx = active_files.len();
        let list_icon_size = self.current_list_icon_size;
        let max_width_chars = self.config.ui.max_width_chars;
        let grid_spacing = self.config.ui.grid_spacing;
        let show_symlink_emblem = self.config.ui.show_symlink_emblem;

        for (offset, item) in results.into_iter().enumerate() {
            let is_dir = item.path.is_dir();
            let icon = utils::icon::get_icon_for_path(&item.path, is_dir);
            let size = item.size;
            let mtime = item.mtime;

            let is_symlink = item.path.is_symlink();
            let is_broken_symlink = is_symlink && !item.path.exists();

            self.tabs[self.active_tab_index].files.append(
                crate::ui::FileItem::builder(item.display, item.path, icon)
                    .icon_size(list_icon_size)
                    .size(size)
                    .mtime(mtime)
                    .is_dir(is_dir)
                    .is_list_mode(true)
                    .grid_idx(start_idx + offset as u32)
                    .max_width_chars(max_width_chars)
                    .grid_spacing(grid_spacing)
                    .is_symlink(is_symlink)
                    .is_broken_symlink(is_broken_symlink)
                    .show_symlink_emblem(show_symlink_emblem)
                    .build(),
            );
        }
    }
}
