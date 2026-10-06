//! Selection queries that map the visual grid selection back to paths.

use crate::model::FluxApp;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Returns a vector of PathBufs representing what the user actually sees as selected.
    ///
    /// If a filter is active, it maps the visual selection indices back to the
    /// matching files. If no filter is active, it maps them directly.
    pub(crate) fn get_selection(&self) -> Vec<PathBuf> {
        let active_files = match self.tabs.get(self.active_tab_index) {
            Some(tab) => &tab.files,
            None => return Vec::new(),
        };

        let selection_model = match active_files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            Some(m) => m,
            None => return Vec::new(),
        };

        let bitset = selection_model.selection();
        let mut selected_paths = Vec::new();

        if let Some((mut iter, first_idx)) = gtk::BitsetIter::init_first(&bitset) {
            let mut visual_indices = Vec::new();
            let mut current = Some(first_idx);
            while let Some(idx) = current {
                visual_indices.push(idx);
                current = iter.next();
            }

            let query_lc = self.filter.to_lowercase();

            if self.filter.is_empty() || self.is_content_searching {
                for idx in visual_indices {
                    if let Some(wrapper) = active_files.get(idx) {
                        selected_paths.push(wrapper.borrow().path.clone());
                    }
                }
            } else if let Some((tags, rest)) = crate::utils::search::parse_tag_filter(&query_lc) {
                let rest_clean = rest.trim().to_lowercase();
                let target_tags: Vec<String> = tags.into_iter().map(|t| t.to_lowercase()).collect();
                let mut match_count = 0u32;

                for i in 0..active_files.len() {
                    if let Some(wrapper) = active_files.get(i) {
                        let item = wrapper.borrow();
                        let name_ok =
                            rest_clean.is_empty() || item.name.to_lowercase().contains(&rest_clean);
                        if name_ok {
                            let file_tags = crate::utils::xattr::read_tags(&item.path);
                            let file_tags_lc: Vec<String> =
                                file_tags.into_iter().map(|t| t.to_lowercase()).collect();
                            if target_tags.iter().all(|req| file_tags_lc.contains(req)) {
                                if visual_indices.contains(&match_count) {
                                    selected_paths.push(item.path.clone());
                                }
                                match_count += 1;
                            }
                        }
                    }
                }
            } else {
                let mut match_count = 0;

                for i in 0..active_files.len() {
                    if let Some(wrapper) = active_files.get(i) {
                        if crate::utils::search::fuzzy_match(&wrapper.borrow().name, &query_lc) {
                            if visual_indices.contains(&(match_count as u32)) {
                                selected_paths.push(wrapper.borrow().path.clone());
                            }
                            match_count += 1;
                        }
                    }
                }
            }
        }
        selected_paths
    }

    /// Returns `(path, is_dir)` pairs for all currently selected items.
    ///
    /// Mirrors [`get_selection`] exactly but preserves the `is_dir` flag from
    /// the [`FileItem`] model. Required when the caller must distinguish virtual
    /// archive directories (whose paths are `archive://` URIs, never real
    /// filesystem paths) from regular files without issuing a syscall.
    pub(crate) fn get_selection_with_meta(&self) -> Vec<(PathBuf, bool)> {
        let active_files = match self.tabs.get(self.active_tab_index) {
            Some(tab) => &tab.files,
            None => return Vec::new(),
        };

        let selection_model = match active_files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            Some(m) => m,
            None => return Vec::new(),
        };

        let bitset = selection_model.selection();
        let mut result = Vec::new();

        if let Some((mut iter, first_idx)) = gtk::BitsetIter::init_first(&bitset) {
            let mut visual_indices = Vec::new();
            let mut current = Some(first_idx);
            while let Some(idx) = current {
                visual_indices.push(idx);
                current = iter.next();
            }

            let query_lc = self.filter.to_lowercase();

            if self.filter.is_empty() || self.is_content_searching {
                for idx in visual_indices {
                    if let Some(wrapper) = active_files.get(idx) {
                        let item = wrapper.borrow();
                        result.push((item.path.clone(), item.is_dir));
                    }
                }
            } else if let Some((tags, rest)) = crate::utils::search::parse_tag_filter(&query_lc) {
                let rest_clean = rest.trim().to_lowercase();
                let target_tags: Vec<String> = tags.into_iter().map(|t| t.to_lowercase()).collect();
                let mut match_count = 0u32;

                for i in 0..active_files.len() {
                    if let Some(wrapper) = active_files.get(i) {
                        let item = wrapper.borrow();
                        let name_ok =
                            rest_clean.is_empty() || item.name.to_lowercase().contains(&rest_clean);
                        if name_ok {
                            let file_tags = crate::utils::xattr::read_tags(&item.path);
                            let file_tags_lc: Vec<String> =
                                file_tags.into_iter().map(|t| t.to_lowercase()).collect();
                            if target_tags.iter().all(|req| file_tags_lc.contains(req)) {
                                if visual_indices.contains(&match_count) {
                                    result.push((item.path.clone(), item.is_dir));
                                }
                                match_count += 1;
                            }
                        }
                    }
                }
            } else {
                let mut match_count = 0u32;

                for i in 0..active_files.len() {
                    if let Some(wrapper) = active_files.get(i) {
                        let item = wrapper.borrow();
                        if crate::utils::search::fuzzy_match(&item.name, &query_lc) {
                            if visual_indices.contains(&match_count) {
                                result.push((item.path.clone(), item.is_dir));
                            }
                            match_count += 1;
                        }
                    }
                }
            }
        }
        result
    }

    /// Returns the filesystem path of the first currently selected item.
    pub(crate) fn get_selected_path(&self) -> Option<PathBuf> {
        self.get_selection().into_iter().next()
    }

    /// During a content search the directory monitor only watches current_path
    /// (flat, non-recursive), so FileDeleted never fires for files in
    /// subdirectories. Remove all grid entries for the selected paths immediately
    /// before handing off to the trash service.
    pub fn remove_search_results_for_paths(&mut self, paths: &[PathBuf]) {
        let mut i = 0;
        while i < self.files.len() {
            if self
                .files
                .get(i)
                .is_some_and(|r| paths.contains(&r.borrow().path))
            {
                self.files.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
