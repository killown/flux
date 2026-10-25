use crate::infra::trash::tracked::{restore_tracked, trash_tracked};
use crate::model::{AppMsg, FluxApp};
use crate::ui::undo_redo::{FileOp, TrashedItem};
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_undo(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_undo");
        if let Some(op) = self.file_op_history.pop_undo() {
            self.execute_undo_op(op, sender);
        }
    }

    pub fn handle_redo(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_redo");
        if let Some(op) = self.file_op_history.pop_redo() {
            self.execute_redo_op(op, sender);
        }
    }

    pub fn handle_undo_move_complete(
        &mut self,
        redo_items: Vec<(PathBuf, PathBuf)>,
        dest_dir: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_undo_move_complete");
        self.file_op_history.push_redo(FileOp::Move {
            items: redo_items,
            dest_dir,
        });
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_undo_move_failed(&mut self, op: FileOp) {
        self.file_op_history.push_undo(op);
    }

    pub fn handle_undo_trash_complete(
        &mut self,
        items: Vec<TrashedItem>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_undo_trash_complete");
        self.file_op_history.push_redo(FileOp::Trash { items });
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_undo_trash_failed(&mut self, op: FileOp) {
        self.file_op_history.push_undo(op);
    }

    pub fn handle_redo_move_complete(
        &mut self,
        items: Vec<(PathBuf, PathBuf)>,
        dest_dir: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_redo_move_complete");
        self.file_op_history
            .push_undo(FileOp::Move { items, dest_dir });
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_redo_move_failed(&mut self, op: FileOp) {
        self.file_op_history.push_redo(op);
    }

    pub fn handle_redo_trash_complete(
        &mut self,
        items: Vec<TrashedItem>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_redo_trash_complete");
        self.file_op_history.push_undo(FileOp::Trash { items });
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_redo_trash_failed(&mut self, op: FileOp) {
        self.file_op_history.push_redo(op);
    }

    fn execute_undo_op(&mut self, op: FileOp, sender: &AsyncComponentSender<Self>) {
        crate::hit!("execute_undo_op");
        match op {
            FileOp::Rename {
                old_path,
                new_path,
                old_name,
                new_name,
            } => {
                if Self::rename_path(&new_path, &old_name).is_ok() {
                    self.file_op_history.push_redo(FileOp::Rename {
                        old_path: new_path,
                        new_path: old_path,
                        old_name: new_name,
                        new_name: old_name,
                    });
                    sender.input(AppMsg::Refresh);
                }
            }
            FileOp::Move { items, dest_dir } => {
                let sender_clone = sender.clone();
                let move_op = FileOp::Move {
                    items: items.clone(),
                    dest_dir: dest_dir.clone(),
                };
                relm4::spawn_blocking(move || {
                    let mut redo_items = Vec::new();
                    let mut success = true;
                    for (src, dst) in items {
                        if crate::app::update::file_ops::paste::perform_file_op(
                            &dst,
                            &src,
                            true,
                            &gtk::gio::Cancellable::new(),
                        )
                        .is_err()
                        {
                            success = false;
                            break;
                        }
                        redo_items.push((src, dst));
                    }
                    if success {
                        sender_clone.input(AppMsg::UndoMoveComplete {
                            redo_items,
                            dest_dir,
                        });
                    } else {
                        sender_clone.input(AppMsg::UndoMoveFailed(move_op));
                    }
                });
            }
            FileOp::Copy { copies, .. } => {
                for path in copies {
                    let _ = std::fs::remove_file(&path).or_else(|_| std::fs::remove_dir_all(&path));
                }
                sender.input(AppMsg::Refresh);
            }
            FileOp::Trash { items } => {
                let sender_clone = sender.clone();
                relm4::spawn_blocking(move || {
                    // Each item is restored from the exact trash entry it
                    // created, and results are reported per item so that a
                    // failure never discards the items already restored.
                    let (restored, failed) = restore_tracked(items);

                    // Failed first: `push_undo` clears the redo stack, so the
                    // redo entry for the restored items has to come after it.
                    if !failed.is_empty() {
                        sender_clone
                            .input(AppMsg::UndoTrashFailed(FileOp::Trash { items: failed }));
                    }
                    if !restored.is_empty() {
                        sender_clone.input(AppMsg::UndoTrashComplete { items: restored });
                    }
                });
            }
        }
    }

    fn execute_redo_op(&mut self, op: FileOp, sender: &AsyncComponentSender<Self>) {
        crate::hit!("execute_redo_op");
        match op {
            FileOp::Rename {
                old_path,
                new_path,
                old_name,
                new_name,
            } => {
                if Self::rename_path(&old_path, &new_name).is_ok() {
                    self.file_op_history.push_undo(FileOp::Rename {
                        old_path: new_path,
                        new_path: old_path,
                        old_name: new_name,
                        new_name: old_name,
                    });
                    sender.input(AppMsg::Refresh);
                }
            }
            FileOp::Move { items, dest_dir } => {
                let sender_clone = sender.clone();
                let move_op = FileOp::Move {
                    items: items.clone(),
                    dest_dir: dest_dir.clone(),
                };
                relm4::spawn_blocking(move || {
                    let mut undo_items = Vec::new();
                    let mut success = true;
                    for (src, dst) in items {
                        if crate::app::update::file_ops::paste::perform_file_op(
                            &src,
                            &dst,
                            true,
                            &gtk::gio::Cancellable::new(),
                        )
                        .is_err()
                        {
                            success = false;
                            break;
                        }
                        undo_items.push((src, dst));
                    }
                    if success {
                        sender_clone.input(AppMsg::RedoMoveComplete {
                            items: undo_items,
                            dest_dir,
                        });
                    } else {
                        sender_clone.input(AppMsg::RedoMoveFailed(move_op));
                    }
                });
            }
            FileOp::Copy { .. } => {}
            FileOp::Trash { items } => {
                let sender_clone = sender.clone();
                relm4::spawn_blocking(move || {
                    let mut re_trashed = Vec::new();
                    let mut failed = Vec::new();

                    for item in items {
                        let file = gio::File::for_path(&item.path);
                        // Re-trashing creates a new trash entry, so record
                        // it: the old entry was consumed by the undo.
                        match trash_tracked(&file, &item.path) {
                            Ok(new_item) => re_trashed.push(new_item),
                            Err(e) => {
                                eprintln!("[Redo] trash failed for {:?}: {}", item.path, e);
                                failed.push(item);
                            }
                        }
                    }

                    // Complete first: `push_undo` clears the redo stack, so
                    // the failed remainder has to be pushed to redo after it.
                    if !re_trashed.is_empty() {
                        sender_clone.input(AppMsg::RedoTrashComplete { items: re_trashed });
                    }
                    if !failed.is_empty() {
                        sender_clone
                            .input(AppMsg::RedoTrashFailed(FileOp::Trash { items: failed }));
                    }
                });
            }
        }
    }
}
