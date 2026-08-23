//! In-memory undo/redo history for session file operations.

use std::path::PathBuf;

const HISTORY_LIMIT: usize = 64;

#[derive(Debug, Clone)]
pub enum FileOp {
    Rename {
        old_path: PathBuf,
        new_path: PathBuf,
        old_name: String,
        new_name: String,
    },
    Move {
        items: Vec<(PathBuf, PathBuf)>,
        dest_dir: PathBuf,
    },
    Copy {
        copies: Vec<PathBuf>,
        #[allow(dead_code)]
        dest_dir: PathBuf,
    },
    Trash {
        paths: Vec<PathBuf>,
    },
}

impl FileOp {
    pub fn label(&self) -> String {
        match self {
            FileOp::Rename {
                old_name, new_name, ..
            } => {
                format!("Rename \"{}\" → \"{}\"", old_name, new_name)
            }
            FileOp::Move { items, .. } => {
                if items.len() == 1 {
                    let name = items[0]
                        .0
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Move \"{}\"", name)
                } else {
                    format!("Move {} items", items.len())
                }
            }
            FileOp::Copy { copies, .. } => {
                if copies.len() == 1 {
                    let name = copies[0]
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Copy \"{}\"", name)
                } else {
                    format!("Copy {} items", copies.len())
                }
            }
            FileOp::Trash { paths } => {
                if paths.len() == 1 {
                    let name = paths[0]
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Trash \"{}\"", name)
                } else {
                    format!("Trash {} items", paths.len())
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct FileOpHistory {
    undo_stack: Vec<FileOp>,
    redo_stack: Vec<FileOp>,
}

impl FileOpHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_undo(&mut self, op: FileOp) {
        self.redo_stack.clear();
        self.undo_stack.push(op);
        if self.undo_stack.len() > HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
    }

    pub fn pop_undo(&mut self) -> Option<FileOp> {
        self.undo_stack.pop()
    }

    pub fn push_redo(&mut self, op: FileOp) {
        self.redo_stack.push(op);
        if self.redo_stack.len() > HISTORY_LIMIT {
            self.redo_stack.remove(0);
        }
    }

    pub fn pop_redo(&mut self) -> Option<FileOp> {
        self.redo_stack.pop()
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    #[allow(dead_code)]
    pub fn undo_label(&self) -> Option<String> {
        self.undo_stack.last().map(FileOp::label)
    }

    #[allow(dead_code)]
    pub fn redo_label(&self) -> Option<String> {
        self.redo_stack.last().map(FileOp::label)
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
