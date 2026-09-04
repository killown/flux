use crate::model::{AppMsg, FluxApp};
use crate::ui::conflict_policy::{
    auto_rename_dest, ConflictChoice, ConflictContext, ConflictPolicy,
};
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// Global operation ID counter, monotonically increasing, unique per session.
pub(crate) static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// Mutex to serialise conflict-resolution dialogs so only one is shown at a time.
static CONFLICT_MUTEX: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

/// Minimum number of files that triggers the dialog without a time delay.
const DIALOG_FILE_THRESHOLD: usize = 5;
/// Delay before showing the dialog for small copies that run longer than expected.
const DIALOG_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

// ─── Public API ───────────────────────────────────────────────────────────────

impl FluxApp {
    pub fn perform_paste_inner(
        &self,
        files: Vec<gio::File>,
        is_cut: bool,
        forced: bool,
        sender: AsyncComponentSender<Self>,
    ) {
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

    /// Core execution batch runner executing paste operations sequentially in one task.
    fn run_paste_batch(
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

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn resolve_gio_files(files: Vec<gio::File>) -> Vec<(PathBuf, String, bool)> {
    files
        .into_iter()
        .filter_map(|file| {
            let src_path = file.path().or_else(|| {
                let uri = file.uri().to_string();
                let clean_uri = uri.trim_end_matches('/');
                gio::File::for_uri(clean_uri).path()
            })?;

            let orig_name = src_path.file_name()?.to_string_lossy().to_string();
            let clean_name = clean_tmp_basename(&orig_name);
            let is_dir = src_path.is_dir();
            Some((src_path, clean_name, is_dir))
        })
        .collect()
}

pub fn perform_file_op(
    src: &Path,
    dest: &Path,
    is_cut: bool,
    cancellable: &gio::Cancellable,
) -> Result<(), String> {
    perform_file_op_with_progress(src, dest, is_cut, cancellable, None)
}

pub fn perform_file_op_with_progress(
    src: &Path,
    dest: &Path,
    is_cut: bool,
    cancellable: &gio::Cancellable,
    mut progress_cb: Option<&mut dyn FnMut(i64, i64)>,
) -> Result<(), String> {
    if src == dest && is_cut {
        return Ok(());
    }

    let src_file = gio::File::for_path(src);
    let dst_file = gio::File::for_path(dest);

    if is_cut {
        let move_res = src_file
            .move_(
                &dst_file,
                gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::ALL_METADATA,
                Some(cancellable),
                match progress_cb.as_mut() {
                    Some(f) => Some(&mut **f),
                    None => None,
                },
            )
            .map_err(|e| e.to_string());

        if move_res.is_ok() {
            return Ok(());
        }
    }

    let copy_res = if src.is_dir() {
        copy_dir_recursive(
            src,
            dest,
            cancellable,
            match progress_cb.as_mut() {
                Some(f) => Some(&mut **f),
                None => None,
            },
        )
        .map_err(|e| e.to_string())
    } else {
        src_file
            .copy(
                &dst_file,
                gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                Some(cancellable),
                match progress_cb.as_mut() {
                    Some(f) => Some(&mut **f),
                    None => None,
                },
            )
            .map_err(|e| e.to_string())
    };

    if copy_res.is_err() {
        if dest.is_file() {
            let _ = std::fs::remove_file(dest);
        } else if dest.is_dir() {
            let _ = std::fs::remove_dir_all(dest);
        }
        return copy_res;
    }

    if is_cut {
        if src.is_dir() {
            let _ = std::fs::remove_dir_all(src);
        } else {
            let _ = std::fs::remove_file(src);
        }
    }

    Ok(())
}

fn copy_dir_recursive(
    src: &Path,
    dest: &Path,
    cancellable: &gio::Cancellable,
    mut progress_cb: Option<&mut dyn FnMut(i64, i64)>,
) -> std::io::Result<()> {
    if src == dest {
        return Ok(());
    }
    if !dest.exists() {
        std::fs::create_dir_all(dest)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;

        if cancellable.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }

        let child_src = entry.path();
        let child_dest = dest.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive(
                &child_src,
                &child_dest,
                cancellable,
                match progress_cb.as_mut() {
                    Some(f) => Some(&mut **f),
                    None => None,
                },
            )?;
        } else {
            gio::File::for_path(&child_src)
                .copy(
                    &gio::File::for_path(&child_dest),
                    gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    Some(cancellable),
                    match progress_cb.as_mut() {
                        Some(f) => Some(&mut **f),
                        None => None,
                    },
                )
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }

    Ok(())
}

fn scan_total_bytes(path: &Path) -> u64 {
    if let Ok(m) = std::fs::metadata(path).or_else(|_| std::fs::symlink_metadata(path)) {
        if m.is_file() || m.is_symlink() {
            return m.len();
        }
        if m.is_dir() {
            let mut total = 0u64;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    total += scan_total_bytes(&entry.path());
                }
            }
            return total;
        }
    }
    0
}

fn clean_tmp_basename(name: &str) -> String {
    if name.starts_with(".tmp") {
        name.split_once('.')
            .and_then(|(_, rest)| rest.split_once('.'))
            .map(|(_, real)| real.to_string())
            .unwrap_or_else(|| name.to_string())
    } else {
        name.to_string()
    }
}

fn is_cancelled_error(cancellable: &gio::Cancellable, msg: &str) -> bool {
    cancellable.is_cancelled()
        || msg.contains("g-io-error-quark: 19")
        || msg.contains("g-io-error-quark:19")
}
