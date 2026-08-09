use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::utils;
use crate::utils::search::{parse_size_filter, SizeOp};
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Resets the filter and view layout state when content search is active.
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
            }
            return;
        }

        // Check for content search trigger: starts with ':'
        if query_lc.starts_with(':') {
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
                    if model.n_items() > 0 {
                        model.select_item(0, true);
                    }
                }
            });
            return;
        }

        // Normal filename filtering
        self.filter = query.clone();
        self.files.clear_filters();
        let filter_text = query_lc.clone();
        self.files
            .add_filter(move |item| item.name.to_lowercase().contains(&filter_text));

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

        // Show relative path tree location if under current_path, else full path
        let relative_path = path
            .strip_prefix(&self.current_path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());

        let display_name = if line_number > 0 {
            format!("{}:{}  {}", relative_path, line_number, line)
        } else {
            format!("{}  ({})", relative_path, line)
        };

        self.files.append(crate::ui::FileItem {
            name: display_name,
            icon,
            thumbnail: None,
            is_dir: false,
            path,
            icon_size: self.current_icon_size,
            size: 0,
            is_editing: false,
            is_foreign_owner: false,
            expand_labels: false,
            is_list_mode: true,
            is_custom_icon: false,
            active_path: std::rc::Rc::new(std::cell::RefCell::new(None)),
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
        if self.header_view == constants::VIEW_SEARCH && self.is_content_searching {
            self.reset_from_content_search();
        } else if self.header_view == constants::VIEW_SEARCH
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
}
