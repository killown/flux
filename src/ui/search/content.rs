use crate::model::FluxApp;
use crate::utils;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Resets search state and restores the pre-search view mode and column layout.
    ///
    /// # Critical Sequence Invariant
    /// The file model (`files.clear()`) must be emptied **before** resetting
    /// `is_list_mode` and updating `view` column boundaries.
    ///
    /// WARNING: If column constraints or list mode sync run while items are still present,
    /// GTK's `GtkListItemFactory` recycles every row and triggers repeated
    /// layout renegotiation passes on the main loop. In list mode, where items
    /// use expanded horizontal layouts, this causes severe UI hangs on large folders.
    pub fn reset_from_content_search(&mut self) {
        if let Some(cancellable) = self.content_search_cancellable.take() {
            cancellable.cancel();
        }
        self.is_content_searching = false;
        self.load_id.fetch_add(1, Ordering::SeqCst);
        self.pending_thumbnails.clear();
        self.filter.clear();
        self.search_just_opened = false;
        self.tabs[self.active_tab_index].files.clear();
        self.is_list_mode = self.saved_list_mode;
        let target_max = if self.is_list_mode {
            1
        } else {
            self.saved_max_columns.max(1)
        };
        let target_min = 1;
        let view = &self.tabs[self.active_tab_index].files.view;
        if view.min_columns() != target_min {
            view.set_min_columns(target_min);
        }
        if view.max_columns() != target_max {
            view.set_max_columns(target_max);
        }
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

        let icon = utils::icon::get_icon_for_path(&path, false);

        let rel_path = path
            .strip_prefix(&self.current_path)
            .map(crate::utils::strip_current_dir)
            .unwrap_or(&path);
        let relative_path = rel_path.to_string_lossy().replace('\0', " ");

        let sanitized_line = line.replace('\0', " ");
        let trimmed_line = sanitized_line.trim();
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

        let active_files = &mut self.tabs[self.active_tab_index].files;
        active_files.append(
            crate::ui::FileItem::builder(relative_path.to_string(), path.clone(), icon)
                .icon_size(self.current_list_icon_size)
                .size(size)
                .mtime(mtime)
                .search_snippet(Some(snippet))
                .is_list_mode(true)
                .grid_idx(active_files.len())
                .max_width_chars(self.config.ui.max_width_chars)
                .grid_spacing(self.config.ui.grid_spacing)
                .show_symlink_emblem(self.config.ui.show_symlink_emblem)
                .line_number(line_number)
                .build(),
        );
    }

    /// Concludes the content search walk and selects the top item.
    pub fn handle_content_search_done(&mut self, session: u64) {
        if self.load_id.load(Ordering::SeqCst) != session {
            return;
        }
        self.is_loading = false;
        self.is_content_searching = false;

        self.content_search_cancellable = None;

        let view = self.tabs[self.active_tab_index].files.view.clone();
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
}
