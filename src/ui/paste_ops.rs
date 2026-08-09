//! Paste / file-copy dispatch logic with active disk-polling progress monitoring.
//!
//! This module replaces the paste functions with real-time file-size watching logic.

use crate::model::{AppMsg, FluxApp};
use crate::services::tasks::TaskQueue;
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{atomic::AtomicUsize, Arc};

// Global operation ID counter, monotonically increasing, unique per session.
pub(crate) static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// Minimum number of files that triggers the dialog without a time delay.
const DIALOG_FILE_THRESHOLD: usize = 5;
/// Minimum total bytes that triggers the dialog without a time delay (32 MiB).
const DIALOG_SIZE_THRESHOLD: u64 = 32 * 1_024 * 1_024;
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
        let target_dir = self.current_path.clone();

        if forced {
            let resolved_files = resolve_gio_files(files);
            self.run_paste_batch(resolved_files, is_cut, target_dir, sender);
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

        self.run_paste_batch(resolved_files, is_cut, target_dir, sender);
    }

    /// Core execution batch runner for both forced and non-conflicting pastes.
    fn run_paste_batch(
        &self,
        resolved_files: Vec<(PathBuf, String, bool)>,
        is_cut: bool,
        target_dir: PathBuf,
        sender: AsyncComponentSender<Self>,
    ) {
        let total_files = resolved_files.len();
        let total_bytes: u64 = resolved_files
            .iter()
            .map(|(p, _, _)| scan_total_bytes(p))
            .sum();

        let completed_files = Arc::new(AtomicUsize::new(0));

        for (src_path, clean_name, is_dir) in resolved_files {
            let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
            let cancellable = gio::Cancellable::new();
            let mut dest = target_dir.join(&clean_name);

            if !is_cut && !is_dir {
                let mut copy_number = 1;
                let original_name = clean_name.clone();

                while dest.exists() {
                    let new_name = match original_name.rfind('.') {
                        Some(idx) if idx > 0 => {
                            let (name, ext) = original_name.split_at(idx);
                            format!("{} (copy {}){}", name, copy_number, ext)
                        }
                        _ => format!("{} (copy {})", original_name, copy_number),
                    };
                    dest = target_dir.join(&new_name);
                    copy_number += 1;
                }
            }

            let file_bytes = scan_total_bytes(&src_path).max(1);

            self.task_queue.update(
                task_id,
                clean_name.clone(),
                0,
                file_bytes,
                total_files,
                cancellable.clone(),
            );

            sender.input(AppMsg::TaskProgress {
                id: task_id,
                label: clean_name.clone(),
                current: 0,
                total: file_bytes,
                total_items: total_files,
                cancellable: cancellable.clone(),
            });

            maybe_show_dialog_immediate(total_files, total_bytes, &sender);

            let s = sender.clone();
            let completed_clone = completed_files.clone();
            let t_queue = self.task_queue.clone();

            {
                let s_delay = s.clone();
                relm4::spawn(async move {
                    tokio::time::sleep(DIALOG_DELAY).await;
                    s_delay.input(AppMsg::ShowTransferDialogIfActive(task_id));
                });
            }

            relm4::spawn_blocking(move || {
                let finished_flag = Arc::new(AtomicBool::new(false));

                spawn_file_watcher(
                    dest.clone(),
                    file_bytes,
                    task_id,
                    clean_name.clone(),
                    total_files,
                    cancellable.clone(),
                    finished_flag.clone(),
                    t_queue.clone(),
                    s.input_sender().clone(),
                );

                let result = perform_file_op(&src_path, &dest, is_cut, &cancellable);

                finished_flag.store(true, Ordering::SeqCst);

                if result.is_ok() {
                    t_queue.update(
                        task_id,
                        clean_name.clone(),
                        file_bytes,
                        file_bytes,
                        total_files,
                        cancellable.clone(),
                    );
                    s.input(AppMsg::TaskProgress {
                        id: task_id,
                        label: clean_name.clone(),
                        current: file_bytes,
                        total: file_bytes,
                        total_items: total_files,
                        cancellable: cancellable.clone(),
                    });

                    if is_cut {
                        s.input(AppMsg::ItemMoved {
                            old_path: src_path.clone(),
                            new_path: dest.clone(),
                        });
                    }
                } else if let Err(ref e) = result {
                    if !is_cancelled_error(e) {
                        s.input(AppMsg::ShowToast(format!("Operation failed: {}", e)));
                    }
                }

                s.input(AppMsg::TaskCompleted(task_id));

                let count = completed_clone.fetch_add(1, Ordering::Relaxed) + 1;
                if count == total_files {
                    s.input(AppMsg::Refresh);
                }
            });
        }
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

#[allow(clippy::too_many_arguments)]
fn spawn_file_watcher(
    dest: PathBuf,
    total_bytes: u64,
    task_id: u64,
    label: String,
    total_items: usize,
    cancellable: gio::Cancellable,
    finished: Arc<AtomicBool>,
    task_queue: Arc<TaskQueue>,
    sender: relm4::Sender<AppMsg>,
) {
    std::thread::spawn(move || {
        while !finished.load(Ordering::SeqCst) && !cancellable.is_cancelled() {
            let current_bytes = scan_total_bytes(&dest).min(total_bytes);

            task_queue.update(
                task_id,
                label.clone(),
                current_bytes,
                total_bytes,
                total_items,
                cancellable.clone(),
            );

            let _ = sender.send(AppMsg::TaskProgress {
                id: task_id,
                label: label.clone(),
                current: current_bytes,
                total: total_bytes,
                total_items,
                cancellable: cancellable.clone(),
            });

            let _ = sender.send(AppMsg::TaskQueueTick);

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

pub fn perform_file_op(
    src: &Path,
    dest: &Path,
    is_cut: bool,
    cancellable: &gio::Cancellable,
) -> Result<(), String> {
    let src_file = gio::File::for_path(src);
    let dst_file = gio::File::for_path(dest);

    if is_cut {
        let move_res = src_file
            .move_(
                &dst_file,
                gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                Some(cancellable),
                None,
            )
            .map_err(|e| e.to_string());

        if move_res.is_ok() {
            return Ok(());
        }
    }

    let copy_res = if src.is_dir() {
        copy_dir_recursive(src, dest, cancellable).map_err(|e| e.to_string())
    } else {
        src_file
            .copy(
                &dst_file,
                gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                Some(cancellable),
                None,
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
            copy_dir_recursive(&child_src, &child_dest, cancellable)?;
        } else {
            gio::File::for_path(&child_src)
                .copy(
                    &gio::File::for_path(&child_dest),
                    gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    Some(cancellable),
                    None,
                )
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }

    Ok(())
}

fn scan_total_bytes(path: &Path) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    if path.is_dir() {
        let mut total = 0u64;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                total += scan_total_bytes(&entry.path());
            }
        }
        return total;
    }
    0
}

fn maybe_show_dialog_immediate(
    file_count: usize,
    total_bytes: u64,
    sender: &AsyncComponentSender<FluxApp>,
) {
    if file_count >= DIALOG_FILE_THRESHOLD || total_bytes >= DIALOG_SIZE_THRESHOLD {
        sender.input(AppMsg::ShowTransferDialog);
    }
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

fn is_cancelled_error(msg: &str) -> bool {
    msg.contains("cancelled")
        || msg.contains("Cancelled")
        || msg.contains("Operation was cancelled")
}
