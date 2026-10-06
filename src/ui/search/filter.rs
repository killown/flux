use crate::model::{AppMsg, FluxApp};
use crate::utils;
use crate::utils::search::{parse_size_filter, SizeOp};
use gtk::glib;
use gtk::prelude::*;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::Utf32Str;
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Resets the filter and view layout state when content search or tag search is active.
    pub fn handle_update_filter(&mut self, query: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_update_filter");
        if query.is_empty() && self.search_just_opened {
            self.search_just_opened = false;
            return;
        }
        let query_lc = query.to_lowercase();
        if query_lc.is_empty() {
            if self.is_content_searching {
                self.reset_from_content_search();
                sender.input(AppMsg::Refresh);
            } else {
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    tab.files.clear_filters();
                }
                sender.input(AppMsg::Refresh);
            }
            return;
        }

        // Check for content search trigger: starts with ':'
        if query_lc.starts_with(':')
            && !query_lc.starts_with(":tag:")
            && !query_lc.starts_with(":t:")
        {
            return;
        }

        // Check for glob pattern (*.iso, etc.) -> Recursive Subfolder Search
        let has_wildcard = query_lc.contains('*') || query_lc.contains('?');
        let has_ext_char = query_lc
            .rfind('.')
            .map(|dot_idx| {
                dot_idx + 1 < query_lc.len() && !query_lc[dot_idx + 1..].trim().is_empty()
            })
            .unwrap_or(false);

        // Instantly lock into list mode if the user starts typing a wildcard pattern
        if has_wildcard {
            if !self.search_saved_layout {
                self.saved_list_mode = self.is_list_mode;
                self.saved_max_columns = self.tabs[self.active_tab_index].files.view.max_columns();
                self.search_saved_layout = true;
            }
            self.is_list_mode = true;
            let active_view = &self.tabs[self.active_tab_index].files.view;
            active_view.set_min_columns(1);
            active_view.set_max_columns(1);
            self.sync_list_mode();
        }

        // Only kick off the heavy recursive search once there are actual characters following the glob/dot
        if has_wildcard && (has_ext_char || query_lc.len() > 1) {
            self.filter = query.clone();
            let query_target = query_lc.clone();

            // Try fast SQLite index query before falling back to full filesystem walk
            let clean_query = query_target.replace(['*', '?'], "");
            let clean_query = clean_query.trim().to_string();
            let current_dir = self.current_path.clone();

            if crate::services::indexer::is_ready() && clean_query.len() >= 3 {
                crate::services::indexer::request_delta_scan();

                if let Ok(index) = crate::services::indexer::SearchIndex::open_or_create() {
                    if let Ok(matches) = index.query_in(&clean_query, Some(&current_dir), 300) {
                        let mut scoped_matches = Vec::new();
                        let mut stale_paths = Vec::new();

                        for p in matches {
                            if p.exists() {
                                scoped_matches.push(p);
                            } else {
                                stale_paths.push(p.to_string_lossy().to_string());
                            }
                        }

                        // Prune dead entries in background if any ghost files were hit
                        if !stale_paths.is_empty() {
                            crate::services::indexer::prune_paths(stale_paths);
                        }

                        if !scoped_matches.is_empty() {
                            // Populate results or feed fast-path view
                        }
                    }
                }
            }

            sender.input(AppMsg::StartAdvancedSearch(
                crate::services::search::AdvancedSearchParams {
                    patterns: vec![query_target],
                    include_hidden: self.show_hidden,
                    ..Default::default()
                },
            ));
            return;
        } else if has_wildcard {
            // Just filter current view or wait while they finish typing the extension (e.g., user typed "*.p")
            self.filter = query.clone();
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                tab.files.clear_filters();
            }
            return;
        }
        // Check for tag filter (:tag:name, :t:name, #name) -> Global Search
        if let Some((tags, rest_query)) = crate::utils::search::parse_tag_filter(&query_lc) {
            self.filter = query.clone();

            let tag_title = format!("#{}", tags.join(", "));
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                tab.title = tag_title.clone();
                if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                    let page = self.tab_view.nth_page(self.active_tab_index as i32);
                    page.set_title(&tag_title);
                }
            }

            let (
                view_clone,
                list_mode,
                list_icon_size,
                current_icon_size,
                max_w,
                spacing,
                show_symlink,
                expand_labels,
                show_empty,
            ) = {
                let tab = &mut self.tabs[self.active_tab_index];
                tab.files.clear_filters();
                tab.files.clear();

                if self.is_list_mode {
                    tab.files.view.set_min_columns(1);
                    tab.files.view.set_max_columns(1);
                } else {
                    tab.files.view.set_min_columns(1);
                    tab.files.view.set_max_columns(20);
                }

                (
                    tab.files.view.clone(),
                    self.is_list_mode,
                    self.current_list_icon_size,
                    self.current_icon_size,
                    self.config.ui.max_width_chars,
                    self.config.ui.grid_spacing,
                    self.config.ui.show_symlink_emblem,
                    self.config.ui.expand_labels,
                    self.config.ui.show_empty_dir_emblem,
                )
            };

            let target_tags: Vec<String> = tags.into_iter().map(|t| t.to_lowercase()).collect();
            let filter_text = rest_query.trim().to_lowercase();

            let mut matching_paths = std::collections::BTreeSet::new();

            for tag in &target_tags {
                if let Ok(paths) = self.state_db.get_paths_for_tag(tag) {
                    for path in paths {
                        if !path.exists() {
                            continue;
                        }

                        // Check if file matches all requested tags
                        let file_tags = crate::utils::xattr::read_tags(&path);
                        let file_tags_lc: Vec<String> =
                            file_tags.into_iter().map(|t| t.to_lowercase()).collect();

                        if target_tags.iter().all(|req| file_tags_lc.contains(req)) {
                            matching_paths.insert(path);
                        }
                    }
                }
            }

            let mut media_tasks: Vec<(u32, std::path::PathBuf)> = Vec::new();
            let active_files = &mut self.tabs[self.active_tab_index].files;
            let mut grid_idx: u32 = active_files.len();

            for path in matching_paths {
                if !filter_text.is_empty() {
                    let filename_str = path
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default();
                    if !filename_str.to_lowercase().contains(&filter_text) {
                        continue;
                    }
                }

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());

                let is_dir = path.is_dir();
                let meta = std::fs::metadata(&path).ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let mtime = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                let icon = utils::icon::get_icon_for_path(&path, is_dir);

                if !is_dir {
                    let (is_img, is_vid) = utils::media::is_visual_media(&path);
                    if is_img || is_vid {
                        media_tasks.push((grid_idx, path.clone()));
                    }
                }

                let is_empty = if is_dir && show_empty {
                    FluxApp::is_dir_empty(&path)
                } else {
                    false
                };

                active_files.append(
                    crate::ui::FileItem::builder(name, path.clone(), icon)
                        .is_dir(is_dir)
                        .icon_size(if list_mode {
                            list_icon_size
                        } else {
                            current_icon_size
                        })
                        .size(size)
                        .mtime(mtime)
                        .is_empty(is_empty)
                        .expand_labels(expand_labels)
                        .is_list_mode(list_mode)
                        .grid_idx(active_files.len())
                        .max_width_chars(max_w)
                        .grid_spacing(spacing)
                        .show_symlink_emblem(show_symlink)
                        .build(),
                );
                grid_idx += 1;
            }

            let session_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
            self.spawn_thumbnail_loader(
                media_tasks,
                session_id,
                self.active_tab_index,
                sender.clone(),
            );

            glib::idle_add_local_once(move || {
                if let Some(model) = view_clone
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                {
                    model.unselect_all();
                }
            });
            return;
        }

        // Check for size filter
        if let Some((size_op, rest_query)) = parse_size_filter(&query_lc) {
            self.filter = query.clone();
            let tab = &mut self.tabs[self.active_tab_index];
            tab.files.clear_filters();

            let filter_text = rest_query.to_lowercase();
            let size_op_clone = size_op.clone();

            tab.files.add_filter(move |item| {
                let name_match =
                    filter_text.is_empty() || item.name.to_lowercase().contains(&filter_text);

                let size_match = if item.is_dir {
                    true
                } else {
                    match size_op_clone {
                        SizeOp::Gt(v) => item.size > v,
                        SizeOp::Lt(v) => item.size < v,
                        SizeOp::Range(l, r) => item.size >= l && item.size <= r,
                    }
                };
                name_match && size_match
            });

            let view = self.tabs[self.active_tab_index].files.view.clone();
            glib::idle_add_local_once(move || {
                if let Some(model) = view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                {
                    model.unselect_all();
                }
            });
            return;
        }

        // Normal filename filtering
        self.filter = query.clone();
        let tab = &mut self.tabs[self.active_tab_index];
        tab.files.clear_filters();
        let view = tab.files.view.clone();

        glib::idle_add_local_once(move || {
            if let Some(model) = view
                .model()
                .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            {
                model.unselect_all();
            }
        });

        let trimmed = query.trim();
        if trimmed.is_empty() {
            return;
        }

        let parsed_pattern = Pattern::parse(trimmed, CaseMatching::Ignore, Normalization::Smart);

        tab.files.add_filter(move |item| {
            let mut target_buf = Vec::new();
            let utf32_target = Utf32Str::new(&item.name, &mut target_buf);
            parsed_pattern
                .score(
                    utf32_target,
                    &mut nucleo::Matcher::new(nucleo::Config::DEFAULT),
                )
                .is_some()
        });
    }
}
