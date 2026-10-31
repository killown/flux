//! In-memory undo/redo history for session file operations.

const HISTORY_LIMIT: usize = 64;

use crate::domain::model::FileOp;

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
