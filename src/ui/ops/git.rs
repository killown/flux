use crate::model::{AppMsg, FileLoadContext, FluxApp};
use crate::services::git::{
    find_git_repo_root, query_git_status, query_git_status_paths, GitFileStatus, StatusFilter,
};
use gtk::prelude::*;
use relm4::AsyncComponentSender;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Opens the right-side diff panel and loads changes for the target file in the background.
    pub fn handle_show_file_diff(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_show_file_diff");
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

    pub fn handle_git_stage_for_commit(
        &mut self,
        paths: Vec<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_git_stage_for_commit");
        let Some(root) = crate::services::git::find_git_repo_root(&self.current_path) else {
            sender.input(AppMsg::ShowToast("Not inside a git repository".to_string()));
            return;
        };

        // Convert absolute selection paths into repo-relative paths so
        // `git add --` can resolve them regardless of cwd.
        let rel: Vec<PathBuf> = paths
            .into_iter()
            .filter_map(|p| p.strip_prefix(&root).ok().map(|r| r.to_path_buf()))
            .collect();

        let s = sender.clone();
        relm4::spawn(async move {
            match crate::services::git::stage_paths(&root, &rel).await {
                Ok(_) => {
                    let _ = s.input_sender().send(AppMsg::GitCommitDialogReady);
                }
                Err(e) => {
                    let _ = s.input_sender().send(AppMsg::GitActionFinished(Err(e)));
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
        crate::hit!("handle_git_status_ready");
        if self.current_path != path {
            return;
        }

        self.git_status_dir = path;
        self.git_status_map = updates;

        // The header Git Actions menu must stay reachable on a clean tree,
        // so is_in_git_repo reflects "inside a repo", not "has changes".
        self.is_in_git_repo = true;

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

    pub fn handle_git_apply_patch(&mut self, patch: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_apply_patch");
        self.spawn_git_action(sender, AppMsg::ShowGitStatusView, move |root| {
            Box::pin(async move { crate::services::git::apply_patch(&root, patch).await })
        });
    }

    pub fn handle_git_stage_all(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_stage_all");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::stage_all(&root).await })
        });
    }

    pub fn handle_git_commit(&mut self, message: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_commit");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::commit(&root, message).await })
        });
    }

    pub fn handle_git_discard_all(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_discard_all");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::discard_all(&root).await })
        });
    }

    pub fn handle_git_unstage_all(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_unstage_all");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::unstage_all(&root).await })
        });
    }

    pub fn handle_git_discard_selected(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_discard_selected");
        let selection = self.get_selection();
        if selection.is_empty() {
            sender.input(AppMsg::ShowToast("Nothing selected".to_string()));
            return;
        }
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move {
                let rel = crate::services::git::repo_relative(&root, selection);
                crate::services::git::discard_selected(&root, &rel).await
            })
        });
    }

    pub fn handle_git_restore_last_discard(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_restore_last_discard");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::restore_last_discard(&root).await })
        });
    }

    pub fn handle_git_undo_apply(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_undo_apply");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::undo_last_apply(&root).await })
        });
    }

    pub fn handle_git_stash(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_stash");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::stash_push(&root).await })
        });
    }

    pub fn handle_git_stash_pop(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_stash_pop");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::stash_pop(&root).await })
        });
    }

    pub fn handle_git_pull(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_pull");
        sender.input(AppMsg::ShowToast("Pulling…".to_string()));
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::pull(&root).await })
        });
    }

    pub fn handle_git_push(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_push");
        sender.input(AppMsg::ShowToast("Pushing…".to_string()));
        self.spawn_git_query(sender, move |root| {
            Box::pin(async move {
                crate::services::git::push(&root)
                    .await
                    .map(AppMsg::ShowToast)
            })
        });
    }

    pub fn handle_git_fetch(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_fetch");
        sender.input(AppMsg::ShowToast("Fetching…".to_string()));
        self.spawn_git_query(sender, move |root| {
            Box::pin(async move {
                crate::services::git::fetch(&root)
                    .await
                    .map(AppMsg::ShowToast)
            })
        });
    }

    pub fn handle_prompt_git_switch_branch(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_prompt_git_switch_branch");
        self.spawn_git_query(sender, move |root| {
            Box::pin(async move {
                let list = crate::services::git::list_branches(&root).await?;
                Ok::<AppMsg, String>(AppMsg::GitBranchesReady {
                    current: list.current,
                    branches: list.branches,
                })
            })
        });
    }

    pub fn handle_git_switch_branch(&mut self, name: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_switch_branch");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::switch_branch(&root, name).await })
        });
    }

    pub fn handle_git_create_branch(&mut self, name: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_create_branch");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::create_branch(&root, name).await })
        });
    }

    pub fn handle_git_copy_diff(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_copy_diff");
        self.spawn_git_query(sender, move |root| {
            Box::pin(async move {
                crate::services::git::diff_head(&root)
                    .await
                    .map(AppMsg::GitCopyDiffReady)
            })
        });
    }

    /// Puts `text` on the clipboard, which must happen on the GTK thread.
    pub fn handle_git_copy_diff_ready(&self, text: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_copy_diff_ready");
        let Some(display) = gtk::gdk::Display::default() else {
            sender.input(AppMsg::ShowToast(
                "No display for the clipboard".to_string(),
            ));
            return;
        };
        display.clipboard().set_text(&text);
        sender.input(AppMsg::ShowToast(format!(
            "Diff copied ({} lines)",
            text.lines().count()
        )));
    }

    pub fn handle_prompt_git_amend(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_prompt_git_amend");
        self.spawn_git_query(sender, move |root| {
            Box::pin(async move {
                crate::services::git::last_commit_message(&root)
                    .await
                    .map(AppMsg::GitAmendDialogReady)
            })
        });
    }

    pub fn handle_git_amend(&mut self, message: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_amend");
        self.spawn_git_action(sender, AppMsg::Refresh, move |root| {
            Box::pin(async move { crate::services::git::amend(&root, message).await })
        });
    }

    /// Selected paths, or the current directory when nothing is selected.
    fn git_targets(&self) -> Vec<PathBuf> {
        let selection = self.get_selection();
        if selection.is_empty() {
            vec![self.current_path.clone()]
        } else {
            selection
        }
    }

    pub fn handle_git_show_log(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_show_log");
        let targets = self.git_targets();
        self.spawn_git_query(sender, move |root| {
            Box::pin(async move {
                let rel = crate::services::git::repo_relative(&root, targets);
                let title = match rel.as_slice() {
                    [] => "Git Log".to_string(),
                    [one] => format!("Git Log: {}", one.display()),
                    many => format!("Git Log: {} paths", many.len()),
                };
                let text = crate::services::git::log_paths(&root, &rel).await?;
                Ok::<AppMsg, String>(AppMsg::GitOutputReady { title, text })
            })
        });
    }

    pub fn handle_git_open_remote(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_git_open_remote");
        let target = self.git_targets().into_iter().next();
        self.spawn_git_query(sender, move |root| {
            Box::pin(async move {
                let rel = target.and_then(|t| {
                    crate::services::git::repo_relative(&root, vec![t])
                        .into_iter()
                        .next()
                });
                crate::services::git::open_remote(&root, rel.as_deref())
                    .await
                    .map(AppMsg::ShowToast)
            })
        });
    }

    /// Runs `job` in the repo root and forwards the message it returns, or reports its error as a toast.
    fn spawn_git_query<F>(&self, sender: &AsyncComponentSender<Self>, job: F)
    where
        F: FnOnce(
                std::path::PathBuf,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<AppMsg, String>> + Send>,
            > + Send
            + 'static,
    {
        let Some(root) = crate::services::git::find_git_repo_root(&self.current_path) else {
            let _ = sender.input_sender().send(AppMsg::GitActionFinished(Err(
                "Not inside a git repository".to_string(),
            )));
            return;
        };

        let s = sender.clone();
        relm4::spawn(async move {
            let msg = match job(root).await {
                Ok(msg) => msg,
                Err(e) => AppMsg::GitActionFinished(Err(e)),
            };
            let _ = s.input_sender().send(msg);
        });
    }

    /// Runs `job` in the repo root, sends `GitActionFinished`, then `on_success` if it worked.
    fn spawn_git_action<F>(&self, sender: &AsyncComponentSender<Self>, on_success: AppMsg, job: F)
    where
        F: FnOnce(
                std::path::PathBuf,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<String, String>> + Send>,
            > + Send
            + 'static,
    {
        let Some(root) = crate::services::git::find_git_repo_root(&self.current_path) else {
            let _ = sender.input_sender().send(AppMsg::GitActionFinished(Err(
                "Not inside a git repository".to_string(),
            )));
            return;
        };

        let s = sender.clone();
        relm4::spawn(async move {
            let result = job(root).await;
            let success = result.is_ok();
            let _ = s.input_sender().send(AppMsg::GitActionFinished(result));
            if success {
                let _ = s.input_sender().send(on_success);
            }
        });
    }

    pub fn handle_git_action_finished(
        &self,
        result: Result<String, String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_git_action_finished");
        // Toasts are meant for short labels, anything long goes to a dialog that can be closed.
        match result {
            Ok(msg) if msg.len() <= 100 && !msg.contains('\n') => {
                sender.input(AppMsg::ShowToast(msg))
            }
            Ok(text) => sender.input(AppMsg::GitOutputReady {
                title: "Git".to_string(),
                text,
            }),
            Err(text) => sender.input(AppMsg::GitOutputReady {
                title: "Git Error".to_string(),
                text,
            }),
        }
    }

    /// Loads only the changed, untracked, and staged git files into the view in deterministic order.
    pub fn handle_show_git_status_view(&mut self, sender: AsyncComponentSender<Self>) {
        crate::hit!("handle_show_git_status_view");
        self.show_git_status_filtered(None, sender);
    }

    /// Same as [`Self::handle_show_git_status_view`], keeping only the staged or unstaged side when `filter` is set.
    pub fn handle_show_git_status_filtered(
        &mut self,
        filter: StatusFilter,
        sender: AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_show_git_status_filtered");
        self.show_git_status_filtered(Some(filter), sender);
    }

    fn show_git_status_filtered(
        &mut self,
        filter: Option<StatusFilter>,
        sender: AsyncComponentSender<Self>,
    ) {
        let target_dir = self.current_path.clone();
        let Some(repo_root) = find_git_repo_root(&target_dir) else {
            return;
        };

        let current_load_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.is_loading = true;
        let s_clone = sender.clone();
        let auto_list_threshold = self.config.ui.auto_list_threshold;
        let current_list_mode = self.is_list_mode;

        relm4::spawn(async move {
            let Ok(status_map) = query_git_status(&repo_root, &target_dir).await else {
                s_clone.input(AppMsg::ShowToast("Failed to query git status".into()));
                return;
            };

            let status_map = match filter {
                None => status_map,
                Some(f) => match query_git_status_paths(&repo_root, f).await {
                    Ok(keep) => status_map
                        .into_iter()
                        .filter(|(path, _)| keep.contains(path))
                        .collect(),
                    Err(e) => {
                        s_clone.input(AppMsg::ShowToast(e));
                        return;
                    }
                },
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

            if contexts.is_empty() {
                match filter {
                    Some(StatusFilter::Staged) => {
                        s_clone.input(AppMsg::ShowToast("No staged changes".into()))
                    }
                    Some(StatusFilter::Unstaged) => {
                        s_clone.input(AppMsg::ShowToast("No unstaged changes".into()))
                    }
                    None => {}
                }
            }

            let list_mode = if auto_list_threshold > 0 && contexts.len() > auto_list_threshold {
                true
            } else {
                current_list_mode
            };

            s_clone.input(AppMsg::FolderLoaded {
                path: target_dir.clone(),
                load_id: current_load_id,
                items: contexts,
                media_tasks: Vec::new(),
                list_mode,
            });

            s_clone.input(AppMsg::GitStatusReady {
                path: target_dir,
                load_id: current_load_id,
                updates: status_updates,
            });
        });
    }
}
