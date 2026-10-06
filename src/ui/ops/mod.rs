//! `FluxApp` operation handlers, grouped by domain.
//!
//! Most modules only add `handle_*` methods to `FluxApp`; `paste` also exposes
//! the shared task-id counter and `perform_file_op`.

pub mod paste;

mod app_action;
pub mod file;
mod git;
mod remote;
mod sidebar;
mod tag;
mod task;
mod terminal;
mod toast;
mod undo;
mod view;
mod watcher;
