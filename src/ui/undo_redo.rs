//! In-memory undo/redo history for session file operations.

use std::path::PathBuf;

const HISTORY_LIMIT: usize = 64;

/// One item that was sent to the system trash.
///
/// `path` is where the item lived before it was trashed. `trash_name` and
/// `deletion_date` identify the specific entry that deletion created in
/// `trash:///`, so undo can restore exactly that entry. Several trashed
/// items can share one original path (delete, recreate, delete again), so
/// the path alone is not enough to tell them apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedItem {
    pub path: PathBuf,
    /// Name of the entry inside `trash:///`. `None` when it could not be
    /// determined right after trashing.
    pub trash_name: Option<String>,
    /// Value of `trash::deletion-date` for the entry, when known.
    pub deletion_date: Option<String>,
}

impl TrashedItem {
    /// An item whose trash entry is unknown. Undo falls back to the newest
    /// trash entry with a matching original path.
    #[allow(dead_code)]
    pub fn untracked(path: PathBuf) -> Self {
        Self {
            path,
            trash_name: None,
            deletion_date: None,
        }
    }
}

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
        items: Vec<TrashedItem>,
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
            FileOp::Trash { items } => {
                if items.len() == 1 {
                    let name = items[0]
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    format!("Trash \"{}\"", name)
                } else {
                    format!("Trash {} items", items.len())
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
