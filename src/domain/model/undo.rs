//! The file operations the session can undo and redo.

use std::path::PathBuf;

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
