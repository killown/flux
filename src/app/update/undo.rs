use crate::infra::trash::tracked::{restore_tracked, trash_tracked_batch};
use crate::model::{AppMsg, FluxApp};
use crate::ui::undo_redo::{FileOp, FileOpHistory, TrashedItem};
use gtk::gio;
use gtk::gio::prelude::FileExt;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Undoes a [`FileOp::Rename`]: moves the item at `new_path` back to
    /// `old_name`, then records `op` on the redo stack.
    ///
    /// A rename op always keeps the orientation it was recorded with
    /// (`old_*` is the name before the rename, `new_*` the name after it), on
    /// both stacks. Undo and redo each read it from that one point of view, so
    /// the fields must never be swapped when the op changes stacks. On error
    /// nothing is pushed.
    pub fn undo_rename(history: &mut FileOpHistory, op: FileOp) -> std::io::Result<()> {
        crate::hit!("undo_rename");
        let FileOp::Rename {
            new_path, old_name, ..
        } = &op
        else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "not a rename operation",
            ));
        };
        Self::rename_path(new_path, old_name)?;
        history.push_redo(op);
        Ok(())
    }

    /// Redoes a [`FileOp::Rename`]: moves the item at `old_path` to
    /// `new_name` again, then records `op` on the undo stack.
    ///
    /// See [`FluxApp::undo_rename`] for why `op` is pushed unchanged.
    pub fn redo_rename(history: &mut FileOpHistory, op: FileOp) -> std::io::Result<()> {
        crate::hit!("redo_rename");
        let FileOp::Rename {
            old_path, new_name, ..
        } = &op
        else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "not a rename operation",
            ));
        };
        Self::rename_path(old_path, new_name)?;
        history.push_undo(op);
        Ok(())
    }

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
            op @ FileOp::Rename { .. } => {
                if Self::undo_rename(&mut self.file_op_history, op).is_ok() {
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
                        if src.symlink_metadata().is_ok()
                            || crate::app::update::file_ops::paste::perform_file_op(
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
                // Undoing a copy sends it to the trash instead of deleting it for
                // good, so edits made to the copy since it was pasted survive.
                for path in &copies {
                    if let Some(parent) = path.parent() {
                        self.folder_cache.remove(&self.cache_key(parent));
                    }
                }
                let sender_clone = sender.clone();
                relm4::spawn_blocking(move || {
                    for path in copies {
                        if path.symlink_metadata().is_err() {
                            continue;
                        }
                        if let Err(e) = gio::File::for_path(&path).trash(gio::Cancellable::NONE) {
                            sender_clone.input(AppMsg::ShowToast(format!(
                                "{}: {}",
                                crate::i18n::tr("Could not move to trash"),
                                e.message()
                            )));
                        }
                    }
                    sender_clone.input(AppMsg::Refresh);
                });
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
            op @ FileOp::Rename { .. } => {
                if Self::redo_rename(&mut self.file_op_history, op).is_ok() {
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
                        if dst.symlink_metadata().is_ok()
                            || crate::app::update::file_ops::paste::perform_file_op(
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
                    let files: Vec<(gio::File, PathBuf)> = items
                        .iter()
                        .map(|item| (gio::File::for_path(&item.path), item.path.clone()))
                        .collect();
                    let (re_trashed, errors) = trash_tracked_batch(&files);
                    let mut failed = Vec::new();
                    for (idx, e) in errors {
                        eprintln!("[Redo] trash failed for {:?}: {}", items[idx].path, e);
                        failed.push(items[idx].clone());
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
