use crate::model::{AppMsg, FileLoadContext, FluxApp};
use crate::services::git::{find_git_repo_root, query_git_status, GitFileStatus};
use gtk::prelude::*;
use relm4::AsyncComponentSender;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Opens the right-side diff panel and loads changes for the target file in the background.
    pub fn handle_show_file_diff(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        if self.active_diff_target.as_ref() == Some(&path) && self.diff_panel_visible {
            return;
        }
        self.active_diff_target = Some(path.clone());
        if !self.diff_panel_visible {
            self.toggle_sidebar_right_panel(crate::model::RightPanelType::Diff, sender);
        }

        let s_clone = sender.clone();
        let target_file = path;
        let current_dir = self.current_path.clone();

        relm4::spawn(async move {
            if let Some(repo_root) = crate::services::git::find_git_repo_root(&current_dir) {
                if let Ok(diff) =
                    crate::services::git::query_file_diff(&repo_root, &target_file).await
                {
                    s_clone.input(AppMsg::DiffLoaded {
                        path: target_file,
                        diff,
                    });
                }
            }
        });
    }

    /// Stores the scanned status for the current directory and rebinds items whose status changed.
    pub fn handle_git_status_ready(
        &mut self,
        path: PathBuf,
        _load_id: u64,
        updates: HashMap<PathBuf, GitFileStatus>,
    ) {
        if self.current_path != path {
            return;
        }

        self.git_status_dir = path;
        self.git_status_map = updates;

        let has_changes = self
            .git_status_map
            .values()
            .any(|&status| status != GitFileStatus::None && status != GitFileStatus::Ignored);
        self.is_in_git_repo = has_changes;

        let Some(tab) = self.tabs.get_mut(self.active_tab_index) else {
            return;
        };

        let mut any_changed = false;
        let total_items = tab.files.len();

        for idx in (0..total_items).rev() {
            let Some(wrapper) = tab.files.get(idx) else {
                continue;
            };
            let new_status = self
                .git_status_map
                .get(&wrapper.borrow().path)
                .copied()
                .unwrap_or_default();

            if wrapper.borrow().git_status != new_status {
                let mut item = wrapper.borrow().clone();
                item.git_status = new_status;
                tab.files.remove(idx);
                tab.files.insert(idx, item);
                any_changed = true;
            }
        }

        if any_changed {
            tab.files.view.queue_draw();
        }
    }

    /// Loads only the changed, untracked, and staged git files into the view in deterministic order.
    pub fn handle_show_git_status_view(&mut self, sender: AsyncComponentSender<Self>) {
        let target_dir = self.current_path.clone();
        let Some(repo_root) = find_git_repo_root(&target_dir) else {
            return;
        };

        let current_load_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.is_loading = true;
        let s_clone = sender.clone();

        relm4::spawn(async move {
            let Ok(status_map) = query_git_status(&repo_root, &target_dir).await else {
                s_clone.input(AppMsg::ShowToast("Failed to query git status".into()));
                return;
            };

            // `is_dir` is stated once per entry and reused by the sort and the loop below.
            let mut filtered_entries: Vec<(PathBuf, GitFileStatus, bool)> = status_map
                .into_iter()
                .filter(|(_, status)| {
                    *status != GitFileStatus::None && *status != GitFileStatus::Ignored
                })
                .map(|(path, status)| {
                    let is_dir = path.is_dir();
                    (path, status, is_dir)
                })
                .collect();

            filtered_entries.sort_by_cached_key(|(path, _, is_dir)| {
                (!*is_dir, path.to_string_lossy().to_lowercase())
            });

            let mut contexts = Vec::with_capacity(filtered_entries.len());
            let mut status_updates = HashMap::with_capacity(filtered_entries.len());

            for (path, status, is_dir) in filtered_entries {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());

                let sort_name = name.to_lowercase();
                let sort_ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();

                let metadata = path.metadata().ok();
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                let mtime = metadata
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                status_updates.insert(path.clone(), status);

                contexts.push(FileLoadContext::with_stats(
                    name, path, is_dir, sort_name, sort_ext, size, mtime, None, false, None,
                ));
            }

            s_clone.input(AppMsg::FolderLoaded {
                path: target_dir.clone(),
                load_id: current_load_id,
                items: contexts,
                media_tasks: Vec::new(),
            });

            s_clone.input(AppMsg::GitStatusReady {
                path: target_dir,
                load_id: current_load_id,
                updates: status_updates,
            });
        });
    }
}
