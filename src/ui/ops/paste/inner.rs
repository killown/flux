use super::transfer::{resolve_gio_files, scan_total_bytes};
use super::{DIALOG_DELAY, DIALOG_FILE_THRESHOLD, NEXT_TASK_ID};
use crate::model::{AppMsg, FluxApp};
use crate::ui::conflict_policy::ConflictPolicy;
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn perform_paste_inner(
        &self,
        files: Vec<gio::File>,
        is_cut: bool,
        forced: bool,
        sender: AsyncComponentSender<Self>,
    ) {
        let current_str = self.current_path.to_string_lossy();

        // ── Archive Paste Intercept ──────────────────────────────────────────
        if current_str.starts_with(crate::services::archive::ARCHIVE_URI)
            || current_str.starts_with("/archive:/")
            || current_str.starts_with("archive://")
        {
            if let Some((archive_path, prefix)) =
                crate::services::archive::parse_archive_uri(&current_str)
            {
                let resolved_sources: Vec<PathBuf> =
                    files.into_iter().filter_map(|f| f.path()).collect();
                let total_files = resolved_sources.len();
                if total_files == 0 {
                    return;
                }

                let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
                let cancellable = gio::Cancellable::new();

                let archive_name = archive_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "archive".to_string());

                let batch_label = if total_files == 1 {
                    resolved_sources[0]
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Adding file".to_string())
                } else {
                    format!("Adding {} items to {}", total_files, archive_name)
                };

                let total_bytes: u64 = resolved_sources.iter().map(|p| scan_total_bytes(p)).sum();

                self.task_queue.update(
                    task_id,
                    batch_label.clone(),
                    0,
                    total_bytes,
                    total_files,
                    cancellable.clone(),
                );

                sender.input(AppMsg::TaskProgress {
                    id: task_id,
                    label: batch_label.clone(),
                    current: 0,
                    total: total_bytes,
                    total_items: total_files,
                    cancellable: cancellable.clone(),
                });
                sender.input(AppMsg::TaskQueueTick);

                if total_files >= DIALOG_FILE_THRESHOLD {
                    sender.input(AppMsg::ShowTransferDialog);
                } else {
                    let s_delay = sender.clone();
                    relm4::spawn(async move {
                        tokio::time::sleep(DIALOG_DELAY).await;
                        s_delay.input(AppMsg::ShowTransferDialogIfActive(task_id));
                    });
                }

                let s = sender.clone();
                let t_queue = self.task_queue.clone();

                relm4::spawn_blocking(move || {
                    let mut copied_bytes: u64 = 0;

                    for (index, src) in resolved_sources.iter().enumerate() {
                        if cancellable.is_cancelled() {
                            break;
                        }

                        let file_size = scan_total_bytes(src);
                        if let Some(name) = src.file_name().and_then(|n| n.to_str()) {
                            let current_label = if total_files > 1 {
                                format!("({}/{}) {}", index + 1, total_files, name)
                            } else {
                                name.to_string()
                            };

                            let inner = if prefix.is_empty() {
                                name.to_string()
                            } else {
                                format!("{}/{}", prefix.trim_end_matches('/'), name)
                            };

                            let t_queue_cb = t_queue.clone();
                            let s_cb = s.clone();
                            let cancellable_cb = cancellable.clone();
                            let label_cb = current_label.clone();
                            let mut current_file_transferred = 0u64;

                            let mut progress_cb = |chunk: u64| {
                                current_file_transferred += chunk;
                                let overall = copied_bytes + current_file_transferred;

                                t_queue_cb.update(
                                    task_id,
                                    label_cb.clone(),
                                    overall,
                                    total_bytes,
                                    total_files,
                                    cancellable_cb.clone(),
                                );
                                s_cb.input(AppMsg::TaskProgress {
                                    id: task_id,
                                    label: label_cb.clone(),
                                    current: overall,
                                    total: total_bytes,
                                    total_items: total_files,
                                    cancellable: cancellable_cb.clone(),
                                });
                                s_cb.input(AppMsg::TaskQueueTick);
                            };

                            if let Err(e) = crate::services::archive::write_archive_entry(
                                &archive_path,
                                src,
                                &inner,
                                None,
                                Some(&mut progress_cb),
                            ) {
                                s.input(AppMsg::ShowToast(format!("Archive error: {e}")));
                                break;
                            }

                            copied_bytes += file_size;
                        }
                    }

                    t_queue.remove(task_id);
                    s.input(AppMsg::TaskCompleted(task_id));
                    s.input(AppMsg::TaskQueueTick);
                    s.input(AppMsg::Refresh);
                });
            }
            return;
        }

        let target_dir = self
            .current_path
            .canonicalize()
            .unwrap_or_else(|_| self.current_path.clone());

        if forced {
            let resolved_files = resolve_gio_files(files);
            self.run_paste_batch(
                resolved_files,
                is_cut,
                target_dir,
                sender,
                ConflictPolicy::ReplaceAll,
            );
        } else {
            self.dispatch_paste_ops(files, is_cut, target_dir, sender);
        }
    }

    pub fn perform_paste(
        &self,
        files: Vec<gio::File>,
        is_cut: bool,
        sender: AsyncComponentSender<Self>,
    ) {
        self.perform_paste_inner(files, is_cut, false, sender);
    }

    /// Shared dispatch logic for non-conflicting paste operations.
    pub fn dispatch_paste_ops(
        &self,
        files: Vec<gio::File>,
        is_cut: bool,
        target_dir: PathBuf,
        sender: AsyncComponentSender<Self>,
    ) {
        let resolved_files = resolve_gio_files(files);
        let mut dir_conflicts = Vec::new();

        for (_src_path, clean_name, is_dir) in &resolved_files {
            if *is_dir {
                let dest = target_dir.join(clean_name);
                if dest.exists() && dest.is_dir() {
                    dir_conflicts.push(clean_name.clone());
                }
            }
        }

        if !dir_conflicts.is_empty() {
            // Legacy folder-conflict dialog - kept as-is.
            let gfiles = resolved_files
                .iter()
                .map(|(p, _, _)| gio::File::for_path(p))
                .collect();

            sender.input(AppMsg::ConfirmReplacePaste {
                files: gfiles,
                conflicts: dir_conflicts,
                is_cut,
            });
            return;
        }

        self.run_paste_batch(
            resolved_files,
            is_cut,
            target_dir,
            sender,
            ConflictPolicy::Ask,
        );
    }
}
