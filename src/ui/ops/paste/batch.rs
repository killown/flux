use super::transfer::{is_cancelled_error, perform_file_op_with_progress, scan_total_bytes};
use super::{CONFLICT_MUTEX, DIALOG_DELAY, DIALOG_FILE_THRESHOLD, NEXT_TASK_ID};
use crate::model::{AppMsg, FluxApp};
use crate::ui::conflict_policy::{
    auto_rename_dest, ConflictChoice, ConflictContext, ConflictPolicy,
};
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

impl FluxApp {
    /// Core execution batch runner executing paste operations sequentially in one task.
    pub(super) fn run_paste_batch(
        &self,
        resolved_files: Vec<(PathBuf, String, bool)>,
        is_cut: bool,
        target_dir: PathBuf,
        sender: AsyncComponentSender<Self>,
        initial_policy: ConflictPolicy,
    ) {
        let total_files = resolved_files.len();
        if total_files == 0 {
            return;
        }

        let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let cancellable = gio::Cancellable::new();

        let batch_label = if total_files == 1 {
            resolved_files[0].1.clone()
        } else {
            format!(
                "{} {} items",
                if is_cut { "Moving" } else { "Copying" },
                total_files
            )
        };

        // Register a single task representing the whole operation
        self.task_queue.update(
            task_id,
            batch_label.clone(),
            0,
            0,
            total_files,
            cancellable.clone(),
        );

        sender.input(AppMsg::TaskProgress {
            id: task_id,
            label: batch_label.clone(),
            current: 0,
            total: 0,
            total_items: total_files,
            cancellable: cancellable.clone(),
        });
        sender.input(AppMsg::TaskQueueTick);

        // Trigger progress dialog if file count or byte threshold is met
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
        let policy = Arc::new(Mutex::new(initial_policy));

        relm4::spawn_blocking(move || {
            // Pre-calculate file sizes once before moving/copying
            let files_with_sizes: Vec<(PathBuf, String, bool, u64)> = resolved_files
                .into_iter()
                .map(|(p, name, is_dir)| {
                    let size = scan_total_bytes(&p);
                    (p, name, is_dir, size)
                })
                .collect();

            let total_bytes: u64 = files_with_sizes.iter().map(|(_, _, _, s)| *s).sum();

            t_queue.update(
                task_id,
                batch_label.clone(),
                0,
                total_bytes,
                total_files,
                cancellable.clone(),
            );
            s.input(AppMsg::TaskQueueTick);

            let mut copied_bytes: u64 = 0;
            let mut successful_ops = Vec::new();

            for (batch_index, (src_path, clean_name, is_dir, file_size)) in
                files_with_sizes.into_iter().enumerate()
            {
                if cancellable.is_cancelled() {
                    break;
                }

                let dest_initial = target_dir.join(&clean_name);

                // Update current item label and progress
                let current_label = if total_files > 1 {
                    format!("({}/{}) {}", batch_index + 1, total_files, clean_name)
                } else {
                    clean_name.clone()
                };

                t_queue.update(
                    task_id,
                    current_label.clone(),
                    copied_bytes,
                    total_bytes,
                    total_files,
                    cancellable.clone(),
                );

                s.input(AppMsg::TaskProgress {
                    id: task_id,
                    label: current_label.clone(),
                    current: copied_bytes,
                    total: total_bytes,
                    total_items: total_files,
                    cancellable: cancellable.clone(),
                });
                s.input(AppMsg::TaskQueueTick);

                // Conflict Resolution
                let dest = if !is_dir && dest_initial.exists() {
                    let current_policy = { policy.lock().unwrap().clone() };
                    match current_policy {
                        ConflictPolicy::ReplaceAll => dest_initial.clone(),
                        ConflictPolicy::SkipAll => {
                            copied_bytes += file_size;
                            continue;
                        }
                        ConflictPolicy::AutoRenameAll => auto_rename_dest(&dest_initial),
                        ConflictPolicy::Ask => {
                            let _lock = CONFLICT_MUTEX.lock();
                            let rechecked = { policy.lock().unwrap().clone() };
                            match rechecked {
                                ConflictPolicy::ReplaceAll => dest_initial.clone(),
                                ConflictPolicy::SkipAll => {
                                    copied_bytes += file_size;
                                    continue;
                                }
                                ConflictPolicy::AutoRenameAll => auto_rename_dest(&dest_initial),
                                ConflictPolicy::Ask => {
                                    let (tx, rx) =
                                        tokio::sync::oneshot::channel::<(ConflictChoice, bool)>();
                                    let ctx = ConflictContext {
                                        src: src_path.clone(),
                                        dest: dest_initial.clone(),
                                        is_cut,
                                        batch_total: total_files,
                                        batch_index: batch_index + 1,
                                    };

                                    s.input(AppMsg::FileConflictDetected {
                                        context: ctx,
                                        resolver: Arc::new(Mutex::new(Some(tx))),
                                    });

                                    let (choice, apply_all) = tokio::runtime::Handle::current()
                                        .block_on(rx)
                                        .unwrap_or((ConflictChoice::Cancel, false));

                                    if apply_all {
                                        let new_policy = match choice {
                                            ConflictChoice::Replace => ConflictPolicy::ReplaceAll,
                                            ConflictChoice::Skip => ConflictPolicy::SkipAll,
                                            ConflictChoice::AutoRename => {
                                                ConflictPolicy::AutoRenameAll
                                            }
                                            ConflictChoice::Cancel => ConflictPolicy::SkipAll,
                                        };
                                        *policy.lock().unwrap() = new_policy;
                                    }

                                    match choice {
                                        ConflictChoice::Cancel => {
                                            cancellable.cancel();
                                            break;
                                        }
                                        ConflictChoice::Skip => {
                                            copied_bytes += file_size;
                                            continue;
                                        }
                                        ConflictChoice::AutoRename => {
                                            auto_rename_dest(&dest_initial)
                                        }
                                        ConflictChoice::Replace => dest_initial.clone(),
                                    }
                                }
                            }
                        }
                    }
                } else {
                    dest_initial.clone()
                };

                // Perform copy/move with byte-level progress reporting
                let bytes_before = copied_bytes;
                let s_cb = s.clone();
                let label_cb = current_label.clone();
                let t_queue_cb = t_queue.clone();
                let cancellable_cb = cancellable.clone();
                let mut progress_cb = move |current_bytes: i64, _total_bytes_file: i64| {
                    let in_flight = current_bytes.max(0) as u64;
                    let overall = bytes_before + in_flight;
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
                let result = perform_file_op_with_progress(
                    &src_path,
                    &dest,
                    is_cut,
                    &cancellable,
                    Some(&mut progress_cb),
                );
                if result.is_ok() {
                    successful_ops.push((src_path.clone(), dest.clone()));
                    copied_bytes += file_size;

                    t_queue.update(
                        task_id,
                        batch_label.clone(),
                        copied_bytes,
                        total_bytes,
                        total_files,
                        cancellable.clone(),
                    );
                    s.input(AppMsg::TaskProgress {
                        id: task_id,
                        label: batch_label.clone(),
                        current: copied_bytes,
                        total: total_bytes,
                        total_items: total_files,
                        cancellable: cancellable.clone(),
                    });
                    s.input(AppMsg::TaskQueueTick);

                    if is_cut {
                        s.input(AppMsg::ItemMoved {
                            old_path: src_path,
                            new_path: dest,
                        });
                    }
                } else if let Err(ref e) = result {
                    if !is_cancelled_error(&cancellable, e) {
                        s.input(AppMsg::ShowToast(format!("Operation failed: {}", e)));
                    }
                }
            }

            t_queue.remove(task_id);
            s.input(AppMsg::TaskCompleted(task_id));
            s.input(AppMsg::TaskQueueTick);

            if !successful_ops.is_empty() {
                if is_cut {
                    s.input(AppMsg::MoveSucceeded {
                        items: successful_ops,
                        dest_dir: target_dir,
                    });
                } else {
                    s.input(AppMsg::CopySucceeded {
                        copies: successful_ops,
                        dest_dir: target_dir,
                    });
                }
                s.input(AppMsg::Refresh);
            }
        });
    }
}
