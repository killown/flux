use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
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
                self.files.clear_filters();
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
                self.saved_max_columns = self.files.view.max_columns();
                self.search_saved_layout = true;
            }
            self.is_list_mode = true;
            self.files.view.set_min_columns(1);
            self.files.view.set_max_columns(1);
            self.sync_list_mode();
        }

        // Only kick off the heavy recursive search once there are actual characters following the glob/dot
        if has_wildcard && (has_ext_char || query_lc.len() > 1) {
            self.filter = query.clone();
            let query_target = query_lc.clone();
            sender.input(AppMsg::StartAdvancedSearch(
                crate::services::extension_search::AdvancedSearchParams {
                    patterns: vec![query_target],
                    include_hidden: self.show_hidden,
                    ..Default::default()
                },
            ));
            return;
        } else if has_wildcard {
            // Just filter current view or wait while they finish typing the extension (e.g., user typed "*.p")
            self.filter = query.clone();
            self.files.clear_filters();
            return;
        }
        // Check for tag filter (:tag:name, :t:name, #name) -> Global Search
        if let Some((tags, rest_query)) = crate::utils::search::parse_tag_filter(&query_lc) {
            self.filter = query.clone();
            self.files.clear_filters();
            self.files.clear();

            if self.is_list_mode {
                self.files.view.set_min_columns(1);
                self.files.view.set_max_columns(1);
            } else {
                self.files.view.set_min_columns(1);
                self.files.view.set_max_columns(20);
            }

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
            let mut grid_idx: u32 = self.files.len();

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

                let icon = utils::get_icon_for_path(&path, is_dir);

                if !is_dir {
                    let (is_img, is_vid) = utils::is_visual_media(&path);
                    if is_img || is_vid {
                        media_tasks.push((grid_idx, path.clone()));
                    }
                }

                let is_empty = if is_dir && self.config.ui.show_empty_dir_emblem {
                    FluxApp::is_dir_empty(&path)
                } else {
                    false
                };

                self.files.append(crate::ui::FileItem {
                    name,
                    icon,
                    thumbnail: None,
                    is_dir,
                    path: path.clone(),
                    icon_size: if self.is_list_mode {
                        self.current_list_icon_size
                    } else {
                        self.current_icon_size
                    },
                    size,
                    mtime,
                    is_editing: false,
                    is_foreign_owner: false,
                    search_snippet: None,
                    is_empty,
                    expand_labels: self.config.ui.expand_labels,
                    is_list_mode: self.is_list_mode,
                    is_custom_icon: false,
                    active_path: std::rc::Rc::new(std::cell::RefCell::new(None)),
                    grid_idx: self.files.len(),
                    max_width_chars: self.config.ui.max_width_chars,
                    grid_spacing: self.config.ui.grid_spacing,
                    is_symlink: false,
                    is_broken_symlink: false,
                    show_symlink_emblem: self.config.ui.show_symlink_emblem,
                    line_number: 0,
                    is_cut: false,
                });
                grid_idx += 1;
            }

            let session_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
            self.spawn_thumbnail_loader(media_tasks, session_id, sender.clone());

            let view = self.files.view.clone();
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

        // Check for size filter
        if let Some((size_op, rest_query)) = parse_size_filter(&query_lc) {
            self.filter = query.clone();
            self.files.clear_filters();

            let filter_text = rest_query.to_lowercase();
            let size_op_clone = size_op.clone();

            self.files.add_filter(move |item| {
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

            let view = self.files.view.clone();
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
        self.files.clear_filters();
        let view = self.files.view.clone();

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

        self.files.add_filter(move |item| {
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

    /// Appends a new content search match result to the grid.
    pub fn handle_content_search_result(
        &mut self,
        path: std::path::PathBuf,
        line: String,
        line_number: usize,
        session: u64,
    ) {
        if self.load_id.load(Ordering::SeqCst) != session {
            return;
        }

        let icon = utils::get_icon_for_path(&path, false);

        let rel_path = path
            .strip_prefix(&self.current_path)
            .map(crate::utils::strip_current_dir)
            .unwrap_or(&path);
        let relative_path = rel_path.to_string_lossy().to_string();

        let trimmed_line = line.trim();
        let snippet = if trimmed_line.chars().count() > 80 {
            let limit_idx = trimmed_line
                .char_indices()
                .nth(80)
                .map(|(idx, _)| idx)
                .unwrap_or(trimmed_line.len());
            let mut s = trimmed_line[..limit_idx].to_string();
            s.push('…');
            s
        } else {
            trimmed_line.to_string()
        };

        let meta = std::fs::metadata(&path).ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        self.files.append(crate::ui::FileItem {
            name: relative_path.to_string(),
            icon,
            thumbnail: None,
            is_dir: false,
            path: path.clone(),
            icon_size: self.current_list_icon_size,
            size,
            mtime,
            is_editing: false,
            is_foreign_owner: false,
            search_snippet: Some(snippet),
            is_empty: false,
            expand_labels: false,
            is_list_mode: true,
            is_custom_icon: false,
            active_path: std::rc::Rc::new(std::cell::RefCell::new(None)),
            grid_idx: self.files.len(),
            max_width_chars: self.config.ui.max_width_chars,
            grid_spacing: self.config.ui.grid_spacing,
            is_symlink: false,
            is_broken_symlink: false,
            show_symlink_emblem: self.config.ui.show_symlink_emblem,
            line_number,
            is_cut: false,
        });
    }

    /// Concludes the content search walk and selects the top item.
    pub fn handle_content_search_done(&mut self, session: u64) {
        if self.load_id.load(Ordering::SeqCst) != session {
            return;
        }
        self.is_loading = false;
        self.is_content_searching = false;

        self.content_search_cancellable = None;

        let view = self.files.view.clone();
        glib::idle_add_local_once(move || {
            if let Some(model) = view
                .model()
                .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            {
                model.unselect_all();
                if model.n_items() > 0 {
                    model.select_item(0, true);
                }
            }
        });
    }

    /// Handles single character keystroke capture for type-to-search.
    pub fn handle_search_input(&mut self, c: char) {
        self.search_just_opened = true;
        self.filter.push(c);
        self.header_view = constants::VIEW_SEARCH.to_string();
    }

    /// Handles backspace press in search entry mode.
    pub fn handle_search_backspace(&mut self, sender: &AsyncComponentSender<Self>) {
        if !self.filter.is_empty() {
            self.filter.pop();
            let query = self.filter.clone();
            sender.input(AppMsg::UpdateFilter(query));
        }
    }

    /// Handles header view stack switches (e.g. search <-> entry <-> path).
    pub fn handle_switch_header(&mut self, view_name: String) {
        if self.header_view == constants::VIEW_SEARCH
            && view_name != constants::VIEW_SEARCH
            && self.is_content_searching
        {
            self.reset_from_content_search();
        } else if self.header_view == constants::VIEW_SEARCH
            && view_name != constants::VIEW_SEARCH
            && !self.is_content_searching
            && self.is_list_mode != self.saved_list_mode
        {
            self.is_list_mode = self.saved_list_mode;
            self.files.view.set_max_columns(self.saved_max_columns);
            self.sync_list_mode();
            self.search_saved_layout = false;
        }
        self.header_view = view_name;
        if self.header_view != constants::VIEW_SEARCH {
            self.filter.clear();
            self.search_just_opened = true;
            self.files.clear_filters();
        }
    }

    #[allow(dead_code)]
    pub fn handle_set_extension_filter(
        &mut self,
        patterns: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let expanded: Vec<String> = patterns
            .iter()
            .flat_map(|p| crate::utils::glob::expand_mime_category(p))
            .collect();
        self.extension_filter = if expanded.is_empty() {
            None
        } else {
            Some(expanded.clone())
        };
        self.extension_globset = crate::utils::glob::compile_patterns(&expanded);
        sender.input(AppMsg::Refresh);
    }

    #[allow(dead_code)]
    pub fn handle_clear_extension_filter(&mut self, sender: &AsyncComponentSender<Self>) {
        self.extension_filter = None;
        self.extension_globset = None;
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_disable_drag_and_drop(&mut self, val: bool) {
        self.config.ui.disable_drag_and_drop = val;
        utils::save_config(&self.config);
    }
}
