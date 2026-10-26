//! Tests for `flux::app` (update handlers, file operations) and the argument parser.

mod args;

mod file_ops {
    mod command_builder;
    mod operations;
    mod paste;
    mod paste_archive;
    mod paste_tmp_name;
    mod paste_undo;
    mod security;
}

// `app_action`, `dialog_prompt`, `navigation`, `sidebar_network`, `watcher`
// and `history` re-implement the logic they assert on instead of calling
// into `flux`; they pass even if the production code is deleted.
mod update {
    mod app_action;
    mod config_handlers;
    mod dialog_prompt;
    mod history;
    mod navigation;
    mod remote_ops;
    mod search_handlers;
    mod sidebar;
    mod sidebar_network;
    mod state;
    mod task_ops;
    mod view_ops;
    mod watcher;
}
