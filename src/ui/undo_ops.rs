use crate::model::{AppMsg, FluxApp};
use crate::ui::undo_redo::FileOp;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_undo(&mut self, sender: &AsyncComponentSender<Self>) {
        if let Some(op) = self.file_op_history.pop_undo() {
            self.execute_undo_op(op, sender);
        }
    }

    pub fn handle_redo(&mut self, sender: &AsyncComponentSender<Self>) {
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
        paths: Vec<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.file_op_history.push_redo(FileOp::Trash { paths });
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
        self.file_op_history
            .push_undo(FileOp::Move { items, dest_dir });
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_redo_move_failed(&mut self, op: FileOp) {
        self.file_op_history.push_redo(op);
    }

    pub fn handle_redo_trash_complete(
        &mut self,
        paths: Vec<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.file_op_history.push_undo(FileOp::Trash { paths });
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_redo_trash_failed(&mut self, op: FileOp) {
        self.file_op_history.push_redo(op);
    }

    fn execute_undo_op(&mut self, op: FileOp, sender: &AsyncComponentSender<Self>) {
        match op {
            FileOp::Rename {
                old_path,
                new_path,
                old_name,
                new_name,
            } => {
                if crate::utils::rename_path(&new_path, &old_name).is_ok() {
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
                        if crate::ui::paste_ops::perform_file_op(
                            &dst,
                            &src,
                            true,
                            &gtk::gio::Cancellable::new(),
                            None,
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
            FileOp::Trash { paths } => {
                let sender_clone = sender.clone();
                let trash_op = FileOp::Trash {
                    paths: paths.clone(),
                };
                relm4::spawn_blocking(move || {
                    let trash_root = gio::File::for_uri("trash:///");
                    let mut restored_paths = Vec::new();
                    let mut success = true;

                    if let Ok(enumerator) = trash_root.enumerate_children(
                        "standard::name,trash::orig-path",
                        gio::FileQueryInfoFlags::NONE,
                        gio::Cancellable::NONE,
                    ) {
                        for child_info in enumerator.flatten() {
                            let orig_path = child_info
                                .attribute_as_string("trash::orig-path")
                                .map(|s| s.to_string())
                                .or_else(|| {
                                    child_info.attribute_byte_string("trash::orig-path").map(
                                        |bytes| {
                                            String::from_utf8_lossy(bytes.as_bytes()).to_string()
                                        },
                                    )
                                });

                            if let Some(orig) = orig_path {
                                if paths.iter().any(|p| p.to_string_lossy() == orig) {
                                    let trash_item = trash_root.child(child_info.name());
                                    let dest = gio::File::for_path(&orig);
                                    let restore_result = trash_item.move_(
                                        &dest,
                                        gio::FileCopyFlags::NONE,
                                        gio::Cancellable::NONE,
                                        None,
                                    );
                                    if restore_result.is_ok() {
                                        restored_paths.push(PathBuf::from(&orig));
                                    } else {
                                        eprintln!("[Restore] move_ failed for {:?}", orig);
                                        success = false;
                                    }
                                }
                            }
                        }
                    }

                    if success && !restored_paths.is_empty() {
                        sender_clone.input(AppMsg::UndoTrashComplete {
                            paths: restored_paths,
                        });
                    } else {
                        sender_clone.input(AppMsg::UndoTrashFailed(trash_op));
                    }
                });
            }
        }
    }

    fn execute_redo_op(&mut self, op: FileOp, sender: &AsyncComponentSender<Self>) {
        match op {
            FileOp::Rename {
                old_path,
                new_path,
                old_name,
                new_name,
            } => {
                if crate::utils::rename_path(&old_path, &new_name).is_ok() {
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
                        if crate::ui::paste_ops::perform_file_op(
                            &src,
                            &dst,
                            true,
                            &gtk::gio::Cancellable::new(),
                            None,
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
            FileOp::Trash { paths } => {
                let sender_clone = sender.clone();
                let trash_op = FileOp::Trash {
                    paths: paths.clone(),
                };
                relm4::spawn_blocking(move || {
                    let mut re_trashed = Vec::new();
                    let mut success = true;

                    for path in &paths {
                        let file = gio::File::for_path(path);
                        if file.trash(gio::Cancellable::NONE).is_ok() {
                            re_trashed.push(path.clone());
                        } else {
                            success = false;
                        }
                    }

                    if success && !re_trashed.is_empty() {
                        sender_clone.input(AppMsg::RedoTrashComplete { paths: re_trashed });
                    } else {
                        sender_clone.input(AppMsg::RedoTrashFailed(trash_op));
                    }
                });
            }
        }
    }
}
