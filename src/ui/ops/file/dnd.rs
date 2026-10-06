use crate::model::{AppMsg, FluxApp};
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Handles drag-and-drop moves for internal items.
    pub fn handle_drop_items(
        &self,
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let dest_str = dest_path.to_string_lossy();

        // ── Archive Drop Intercept ───────────────────────────────────────────
        if dest_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            if let Some((archive_path, prefix)) =
                crate::services::archive::parse_archive_uri(&dest_str)
            {
                let s_clone = sender.clone();
                relm4::spawn_blocking(move || {
                    for src in source_paths {
                        if let Some(file_name) = src.file_name().and_then(|n| n.to_str()) {
                            let inner = if prefix.is_empty() {
                                file_name.to_string()
                            } else {
                                format!("{}/{}", prefix.trim_end_matches('/'), file_name)
                            };

                            if let Err(e) = crate::services::archive::write_archive_entry(
                                &archive_path,
                                &src,
                                &inner,
                                None,
                                None,
                            ) {
                                s_clone.input(AppMsg::ShowToast(format!("Archive error: {e}")));
                                return;
                            }
                        }
                    }
                    s_clone.input(AppMsg::Refresh);
                });
            }
            return;
        }

        if dest_path.as_os_str().is_empty() || !dest_path.is_dir() {
            return;
        }

        let Ok(dest_canon) = dest_path.canonicalize() else {
            return;
        };

        let sender_clone = sender.clone();

        relm4::spawn_blocking(move || {
            let mut completed_moves = Vec::new();

            for source_path in source_paths {
                let Ok(source_canon) = source_path.canonicalize() else {
                    continue;
                };

                // HARD GUARD: If the file's parent directory is the exact same as the destination, skip!
                if let Some(parent) = source_canon.parent() {
                    if parent == dest_canon {
                        continue;
                    }
                }

                if source_canon == dest_canon {
                    continue;
                }

                let Some(file_name) = source_canon.file_name() else {
                    continue;
                };

                let final_dest = dest_canon.join(file_name);
                if source_canon == final_dest {
                    continue;
                }

                let src_file = gio::File::for_path(&source_canon);
                let dst_file = gio::File::for_path(&final_dest);

                if src_file
                    .move_(
                        &dst_file,
                        gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::ALL_METADATA,
                        gio::Cancellable::NONE,
                        None,
                    )
                    .is_ok()
                {
                    completed_moves.push((source_canon.clone(), final_dest.clone()));
                    sender_clone.input(AppMsg::ItemMoved {
                        old_path: source_canon,
                        new_path: final_dest,
                    });
                } else {
                    eprintln!("[DnD Error] Failed to move {:?}", source_canon);
                }
            }

            if !completed_moves.is_empty() {
                sender_clone.input(AppMsg::MoveSucceeded {
                    items: completed_moves,
                    dest_dir: dest_canon,
                });
            }

            sender_clone.input(AppMsg::Refresh);
        });
    }

    /// Handles drag-and-drop moves from external windows/processes.
    pub fn handle_external_drop_items(
        &self,
        source_paths: Vec<PathBuf>,
        _dest_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        // FORCE the destination to always be the directory currently viewed in Flux,
        // preventing window-level drop targets from accidentally dumping files into /home/neo.
        let dest_path = self.current_path.clone();

        if dest_path.as_os_str().is_empty() || !dest_path.is_dir() {
            return;
        }

        let Ok(dest_canon) = dest_path.canonicalize() else {
            return;
        };

        let sender_clone = sender.clone();
        relm4::spawn_blocking(move || {
            let mut completed_moves = Vec::new();

            for source in source_paths {
                let Ok(source_canon) = source.canonicalize() else {
                    continue;
                };

                // HARD GUARD: If the file is already in the current folder, do not move it!
                if let Some(parent) = source_canon.parent() {
                    if parent == dest_canon {
                        continue;
                    }
                }

                if source_canon == dest_canon {
                    continue;
                }

                let Some(file_name) = source_canon.file_name() else {
                    continue;
                };

                let final_dest = dest_canon.join(file_name);

                if source_canon == final_dest {
                    continue;
                }

                let src_file = gio::File::for_path(&source_canon);
                let dst_file = gio::File::for_path(&final_dest);

                if src_file
                    .move_(
                        &dst_file,
                        gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::ALL_METADATA,
                        gio::Cancellable::NONE,
                        None,
                    )
                    .is_ok()
                {
                    completed_moves.push((source_canon.clone(), final_dest.clone()));
                    sender_clone.input(AppMsg::ItemMoved {
                        old_path: source_canon,
                        new_path: final_dest,
                    });
                } else {
                    eprintln!("[File Error] External move failed");
                }
            }

            if !completed_moves.is_empty() {
                sender_clone.input(AppMsg::MoveSucceeded {
                    items: completed_moves,
                    dest_dir: dest_canon,
                });
            }

            sender_clone.input(AppMsg::Refresh);
        });
    }
}
