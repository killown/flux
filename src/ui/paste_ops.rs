//! Paste / file-copy dispatch logic with GIO per-byte progress callbacks.
//!
//! This module replaces the three paste functions that previously lived in
//! the large `FluxApp` impl block inside `ui/`. Drop-in: the public signatures
//! of `perform_paste_inner`, `perform_paste`, and `dispatch_paste_ops` are unchanged.

use crate::model::{AppMsg, FluxApp};
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{atomic::AtomicUsize, Arc};

// Global operation ID counter, monotonically increasing, unique per session.
pub(crate) static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// Minimum number of files that triggers the dialog without a time delay.
const DIALOG_FILE_THRESHOLD: usize = 5;
/// Minimum total bytes that triggers the dialog without a time delay (32 MiB).
const DIALOG_SIZE_THRESHOLD: u64 = 32 * 1_024 * 1_024;
/// Delay before showing the dialog for small copies that run longer than expected.
const DIALOG_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

// ─── Public API (mirrors the original signatures exactly) ─────────────────────

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
            let total_files = files.len();
            let completed = Arc::new(AtomicUsize::new(0));

            // Pre-scan total bytes so the dialog denominator is correct.
            let total_bytes: u64 = files
                .iter()
                .filter_map(|f| f.path())
                .map(|p| scan_total_bytes(&p))
                .sum();

            for file in files {
                let src = match file.path().or_else(|| {
                    let uri = file.uri().to_string();
                    let clean_uri = uri.trim_end_matches('/');
                    gio::File::for_uri(clean_uri).path()
                }) {
                    Some(p) => p,
                    None => continue,
                };

                let orig_basename = match src.file_name() {
                    Some(f) => f.to_string_lossy().to_string(),
                    None => continue,
                };

                let clean_basename = clean_tmp_basename(&orig_basename);
                let dest = target_dir.join(&clean_basename);

                let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
                let cancellable = gio::Cancellable::new();

                sender.input(AppMsg::TaskProgress {
                    id: task_id,
                    label: clean_basename.clone(),
                    current: 0,
                    total: total_bytes.max(1),
                    total_items: total_files,
                    cancellable: cancellable.clone(),
                });

                // Show dialog immediately for large / multi-file operations.
                maybe_show_dialog_immediate(total_files, total_bytes, &sender);

                let s = sender.clone();
                let completed_clone = completed.clone();

                relm4::spawn_blocking(move || {
                    let result = if is_cut {
                        let file_bytes = scan_total_bytes(&src).max(1);

                        let move_result = gio::File::for_path(&src)
                            .move_(
                                &gio::File::for_path(&dest),
                                gio::FileCopyFlags::OVERWRITE
                                    | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                                Some(&cancellable),
                                None,
                            )
                            .map_err(|e| e.to_string());

                        if move_result.is_ok() {
                            s.input(AppMsg::TaskProgress {
                                id: task_id,
                                label: clean_basename.clone(),
                                current: file_bytes,
                                total: file_bytes,
                                total_items: total_files,
                                cancellable: cancellable.clone(),
                            });

                            s.input(AppMsg::ItemMoved {
                                old_path: src.clone(),
                                new_path: dest.clone(),
                            });
                        }

                        move_result
                    } else if src.is_dir() {
                        copy_dir_recursive_progress(
                            &src,
                            &dest,
                            &cancellable,
                            task_id,
                            s.input_sender(),
                        )
                        .map_err(|e| e.to_string())
                    } else {
                        copy_file_progress(&src, &dest, &cancellable, task_id, s.input_sender())
                            .map_err(|e| e.to_string())
                    };

                    if let Err(e) = result {
                        if !is_cancelled_error(&e) {
                            s.input(AppMsg::ShowToast(format!("Copy failed: {}", e)));
                        }
                    }

                    s.input(AppMsg::TaskCompleted(task_id));

                    let count = completed_clone.fetch_add(1, Ordering::Relaxed) + 1;
                    if count == total_files {
                        s.input(AppMsg::Refresh);
                    }
                });
            }
        } else {
            Self::dispatch_paste_ops(files, is_cut, target_dir, sender);
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
        files: Vec<gio::File>,
        is_cut: bool,
        target_dir: PathBuf,
        sender: AsyncComponentSender<Self>,
    ) {
        let mut dir_conflicts = Vec::new();

        let resolved_files: Vec<(PathBuf, String, bool)> = files
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
            .collect();

        for (_, name, is_dir) in &resolved_files {
            if *is_dir {
                let dest = target_dir.join(name);
                if dest.exists() && dest.is_dir() {
                    dir_conflicts.push(name.clone());
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

        let total_files = resolved_files.len();

        // Pre-scan total bytes before spawning any I/O.
        let total_bytes: u64 = resolved_files
            .iter()
            .map(|(p, _, _)| scan_total_bytes(p))
            .sum();

        let completed_files = Arc::new(AtomicUsize::new(0));

        for (src_path, clean_name, is_dir) in resolved_files {
            let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
            let cancellable = gio::Cancellable::new();

            let mut dest = target_dir.join(&clean_name);

            // Avoid silently overwriting existing files for copy (not cut).
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
                    dest = target_dir.join(new_name);
                    copy_number += 1;
                }
            }

            sender.input(AppMsg::TaskProgress {
                id: task_id,
                label: clean_name.clone(),
                current: 0,
                total: total_bytes.max(1),
                total_items: total_files,
                cancellable: cancellable.clone(),
            });

            maybe_show_dialog_immediate(total_files, total_bytes, &sender);

            let s = sender.clone();
            let completed_clone = completed_files.clone();

            {
                let s_delay = s.clone();
                relm4::spawn(async move {
                    tokio::time::sleep(DIALOG_DELAY).await;
                    s_delay.input(AppMsg::ShowTransferDialogIfActive(task_id));
                });
            }

            relm4::spawn_blocking(move || {
                let result = if is_cut {
                    let file_bytes = scan_total_bytes(&src_path).max(1);

                    let move_result = gio::File::for_path(&src_path)
                        .move_(
                            &gio::File::for_path(&dest),
                            gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                            Some(&cancellable),
                            None,
                        )
                        .map_err(|e| e.to_string());

                    if move_result.is_ok() {
                        s.input(AppMsg::TaskProgress {
                            id: task_id,
                            label: clean_name.clone(),
                            current: file_bytes,
                            total: file_bytes,
                            total_items: total_files,
                            cancellable: cancellable.clone(),
                        });

                        s.input(AppMsg::ItemMoved {
                            old_path: src_path.clone(),
                            new_path: dest.clone(),
                        });
                    }

                    move_result
                } else if is_dir {
                    copy_dir_recursive_progress(
                        &src_path,
                        &dest,
                        &cancellable,
                        task_id,
                        s.input_sender(),
                    )
                    .map_err(|e| e.to_string())
                } else {
                    copy_file_progress(&src_path, &dest, &cancellable, task_id, s.input_sender())
                        .map_err(|e| e.to_string())
                };

                if let Err(e) = result {
                    if !is_cancelled_error(&e) {
                        s.input(AppMsg::ShowToast(format!("Copy failed: {}", e)));
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

fn copy_file_progress(
    src: &Path,
    dest: &Path,
    cancellable: &gio::Cancellable,
    task_id: u64,
    sender: &relm4::Sender<AppMsg>,
) -> Result<(), glib::Error> {
    let src_file = gio::File::for_path(src);
    let dst_file = gio::File::for_path(dest);
    let label = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let s = sender.clone();
    let lbl = label.clone();

    let mut progress_cb = move |current: i64, total: i64| {
        let _ = s.send(AppMsg::TaskProgress {
            id: task_id,
            label: lbl.clone(),
            current: current as u64,
            total: total as u64,
            total_items: 1,
            cancellable: gio::Cancellable::new(),
        });
    };

    src_file.copy(
        &dst_file,
        gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
        Some(cancellable),
        Some(&mut progress_cb),
    )
}

fn copy_dir_recursive_progress(
    src: &Path,
    dest: &Path,
    cancellable: &gio::Cancellable,
    task_id: u64,
    sender: &relm4::Sender<AppMsg>,
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
            copy_dir_recursive_progress(&child_src, &child_dest, cancellable, task_id, sender)?;
        } else {
            let label = child_src
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let s = sender.clone();
            let lbl = label.clone();
            let c = cancellable.clone();

            let mut progress_cb = move |current: i64, total: i64| {
                let _ = s.send(AppMsg::TaskProgress {
                    id: task_id,
                    label: lbl.clone(),
                    current: current as u64,
                    total: total as u64,
                    total_items: 1,
                    cancellable: c.clone(),
                });
            };

            gio::File::for_path(&child_src)
                .copy(
                    &gio::File::for_path(&child_dest),
                    gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    Some(cancellable),
                    Some(&mut progress_cb),
                )
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }

    Ok(())
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
