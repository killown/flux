use crate::model::{FluxApp, SortBy};
use adw::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn sort_status(&self) -> &str {
        match self.sort_by {
            SortBy::Name => "Name",
            SortBy::Date => "Date",
            SortBy::Size => "Size",
        }
    }

    pub(crate) fn get_selected_path(&self) -> Option<PathBuf> {
        self.files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            .and_then(|selection_model| {
                let selection = selection_model.selection();
                if selection.is_empty() {
                    return None;
                }
                let first_index = selection.nth(0);
                self.files
                    .get(first_index)
                    .map(|wrapper| wrapper.borrow().path.clone())
            })
    }
}
