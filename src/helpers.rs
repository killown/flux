use crate::model::{FluxApp, PathSegment, SortBy};
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

    pub fn update_breadcrumbs(&mut self) {
        let mut guard = self.breadcrumbs.guard();
        guard.clear();

        let path_str = self.current_path.to_string_lossy();

        // Handle Virtual Trash Path
        if path_str.starts_with("trash://") {
            guard.push_back(PathSegment {
                name: "Trash".to_string(),
                path: PathBuf::from("trash:///"),
            });
            return;
        }

        // Standard Filesystem Path Logic
        let mut components = Vec::new();
        let mut ancestor = self.current_path.as_path();

        while let Some(parent) = ancestor.parent() {
            if let Some(name) = ancestor.file_name() {
                components.push(PathSegment {
                    name: name.to_string_lossy().to_string(),
                    path: ancestor.to_path_buf(),
                });
            }
            ancestor = parent;
        }

        components.push(PathSegment {
            name: "/".to_string(),
            path: PathBuf::from("/"),
        });

        for segment in components.into_iter().rev() {
            guard.push_back(segment);
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
