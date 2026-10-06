use crate::model::{AppMsg, FluxApp, SortBy};
use crate::ui::constants;
use crate::utils;
use relm4::prelude::*;

impl FluxApp {
    /// Toggles between grid card layout and compact list view.
    pub fn handle_toggle_list_mode(&mut self) {
        self.is_list_mode = !self.is_list_mode;
        self.saved_list_mode = self.is_list_mode;
        self.config.default_list_mode = self.is_list_mode;
        utils::save_config(&self.config);

        let is_list = self.is_list_mode;
        let new_icon_size = if is_list {
            self.current_list_icon_size
        } else {
            self.current_icon_size
        };

        let active_tab = match self.tabs.get_mut(self.active_tab_index) {
            Some(t) => t,
            None => return,
        };

        if is_list {
            active_tab.files.view.set_min_columns(1);
            active_tab.files.view.set_max_columns(1);
        } else {
            active_tab.files.view.set_min_columns(1);
            active_tab.files.view.set_max_columns(20);
        }

        for i in 0..active_tab.files.len() {
            if let Some(item_wrapper) = active_tab.files.get(i) {
                let mut item = item_wrapper.borrow().clone();
                item.is_list_mode = is_list;
                item.icon_size = new_icon_size;
                active_tab.files.remove(i);
                active_tab.files.insert(i, item);
            }
        }

        self.sync_list_mode();
    }

    /// Toggles between ascending and descending sort order.
    pub fn handle_toggle_sort_order(&mut self, sender: &AsyncComponentSender<Self>) {
        self.sort_ascending = !self.sort_ascending;
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Refresh);
    }

    /// Cycles through available sorting criteria (Name -> Date -> Size -> Type).
    pub fn handle_cycle_sort(&mut self, sender: &AsyncComponentSender<Self>) {
        self.sort_by = match self.sort_by {
            SortBy::Name => SortBy::Date,
            SortBy::Date => SortBy::Size,
            SortBy::Size => SortBy::Type,
            SortBy::Type => SortBy::Name,
        };
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Refresh);
    }

    /// Toggles "Folders First" sorting priority for the current directory view.
    pub fn handle_cycle_folder_priority(&mut self, sender: &AsyncComponentSender<Self>) {
        let path = self.current_path.clone();
        let current_state = if let Ok(Some((_, _, _, ff))) = self.state_db.get_view(&path) {
            ff
        } else {
            self.config.ui.folders_first
        };
        let new_state = !current_state;

        let _ = self.state_db.save_view(
            &path,
            &format!("{:?}", self.sort_by),
            false,
            self.current_icon_size as u32,
            new_state,
        );
        self.load_path(path, sender);
    }

    /// Adjusts icon scale on scroll, targeting only the active view mode.
    ///
    /// In grid mode the persistent `current_icon_size` and `config.ui.default_icon_size`
    /// are updated. In list mode `current_list_icon_size` and `config.ui.list_icon_size`
    /// are updated instead. Only items matching the current mode have their `icon_size`
    /// field mutated, so switching modes always restores the independent size.
    pub fn handle_zoom(&mut self, delta: f64) {
        crate::hit!("handle_zoom");
        if delta == 0.0 {
            return;
        }

        let direction = if delta > 0.0 { -1 } else { 1 };

        if self.is_list_mode {
            let current_size = self.current_list_icon_size;
            let current_idx = constants::CRISP_ICON_SIZES
                .iter()
                .position(|&s| s >= current_size)
                .unwrap_or(constants::CRISP_ICON_SIZES.len() - 1);

            let new_idx = if direction > 0 {
                (current_idx + 1).min(constants::CRISP_ICON_SIZES.len() - 1)
            } else {
                current_idx.saturating_sub(1)
            };

            let new_size = constants::CRISP_ICON_SIZES[new_idx];
            if new_size == self.current_list_icon_size {
                return;
            }

            self.current_list_icon_size = new_size;
            self.config.ui.list_icon_size = new_size;
            utils::save_config(&self.config);

            let active_files = &mut self.tabs[self.active_tab_index].files;
            for i in 0..active_files.len() {
                if let Some(item_wrapper) = active_files.get(i) {
                    if item_wrapper.borrow().is_list_mode {
                        let mut item = item_wrapper.borrow().clone();
                        item.icon_size = new_size;
                        active_files.remove(i);
                        active_files.insert(i, item);
                    }
                }
            }
        } else {
            let current_size = self.current_icon_size;
            let current_idx = constants::CRISP_ICON_SIZES
                .iter()
                .position(|&s| s >= current_size)
                .unwrap_or(constants::CRISP_ICON_SIZES.len() - 1);

            let new_idx = if direction > 0 {
                (current_idx + 1).min(constants::CRISP_ICON_SIZES.len() - 1)
            } else {
                current_idx.saturating_sub(1)
            };

            let new_size = constants::CRISP_ICON_SIZES[new_idx];
            if new_size == self.current_icon_size {
                return;
            }

            self.current_icon_size = new_size;
            let _ = self.state_db.save_view(
                &self.current_path,
                &format!("{:?}", self.sort_by),
                false,
                new_size as u32,
                self.config.ui.folders_first,
            );

            let active_files = &mut self.tabs[self.active_tab_index].files;
            for i in 0..active_files.len() {
                if let Some(item_wrapper) = active_files.get(i) {
                    if !item_wrapper.borrow().is_list_mode {
                        let mut item = item_wrapper.borrow().clone();
                        item.icon_size = new_size;
                        active_files.remove(i);
                        active_files.insert(i, item);
                    }
                }
            }
        }
    }
}
