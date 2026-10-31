//! Undo and redo.

use crate::domain::model::{FileOp, TrashedItem};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum UndoMsg {
    /// Undo the most recent file operation (Ctrl+Z).
    Undo,
    /// Redo the most recently undone operation (Ctrl+Shift+Z / Ctrl+Y).
    Redo,
    /// Internal: background undo-move succeeded, record the inverse redo op.
    UndoMoveComplete {
        redo_items: Vec<(std::path::PathBuf, std::path::PathBuf)>,
        dest_dir: std::path::PathBuf,
    },
    /// Internal: background undo-move failed, restore op on the undo stack.
    UndoMoveFailed(FileOp),
    /// Internal: background undo-trash succeeded, record the inverse redo op.
    UndoTrashComplete { items: Vec<TrashedItem> },
    /// Internal: background undo-trash failed, restore op on the undo stack.
    UndoTrashFailed(FileOp),
    /// Internal: background redo-move succeeded, record the inverse undo op.
    RedoMoveComplete {
        items: Vec<(std::path::PathBuf, std::path::PathBuf)>,
        dest_dir: std::path::PathBuf,
    },
    /// Internal: background redo-move failed, restore op on the redo stack.
    RedoMoveFailed(FileOp),
    /// Internal: background redo-trash succeeded, record the inverse undo op.
    RedoTrashComplete { items: Vec<TrashedItem> },
    /// Internal: background redo-trash failed, restore op on the redo stack.
    RedoTrashFailed(FileOp),
}
