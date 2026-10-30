use crate::domain::model::{
    AppMsg, FileOpsMsg, FluxApp, NavigationMsg, PropertiesMsg, QuickListMsg, UndoMsg, WindowMsg,
};
use relm4::prelude::*;

mod archive;
mod clipboard;
mod command;
mod dnd;
mod link;
pub mod paste;
mod rename;
pub mod system;
mod trash;

// Public so integration tests (and other crates) can reach the command builder.
#[allow(unused_imports)]
pub use command::{build_execution_command, should_track_in_transfer_dialog};

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
    match msg {
        AppMsg::FileOps(FileOpsMsg::CopyPath) => app.handle_copy_path(sender),
        AppMsg::FileOps(FileOpsMsg::CopyPathReady(text)) => {
            app.handle_copy_path_ready(text, sender)
        }
        AppMsg::FileOps(FileOpsMsg::CreateSymlink) => app.handle_create_link(false, sender),
        AppMsg::FileOps(FileOpsMsg::CreateHardlink) => app.handle_create_link(true, sender),
        AppMsg::FileOps(FileOpsMsg::Copy) => app.handle_copy_or_cut(false, sender),
        AppMsg::FileOps(FileOpsMsg::Cut) => app.handle_copy_or_cut(true, sender),
        AppMsg::FileOps(FileOpsMsg::Paste) => app.handle_paste_from_clipboard(sender),
        AppMsg::FileOps(FileOpsMsg::PasteImageFromClipboard)
        | AppMsg::FileOps(FileOpsMsg::PasteTextFromClipboard)
        | AppMsg::FileOps(FileOpsMsg::PasteHtmlFromClipboard) => {}
        AppMsg::FileOps(FileOpsMsg::ConfirmReplacePaste {
            files,
            conflicts,
            is_cut,
        }) => app.show_confirm_replace_paste(files, conflicts, is_cut, sender),
        AppMsg::FileOps(FileOpsMsg::PerformPaste { files, is_cut }) => {
            app.perform_paste(files, is_cut, sender.clone())
        }
        AppMsg::FileOps(FileOpsMsg::PerformPasteForced { files, is_cut }) => {
            app.perform_paste_inner(files, is_cut, true, sender.clone())
        }
        AppMsg::FileOps(FileOpsMsg::PerformRename(old_path, new_name)) => {
            app.handle_perform_rename(old_path, new_name, sender)
        }
        AppMsg::FileOps(FileOpsMsg::InstallFonts(paths)) => {
            let s = sender.clone();
            relm4::spawn_blocking(move || {
                let report = crate::infra::fonts::install_fonts(&paths);
                s.input(AppMsg::Window(WindowMsg::ShowToast(report.message())));
            });
        }
        AppMsg::FileOps(FileOpsMsg::Delete) => {
            let selection = app.get_selection();
            if app.is_content_searching {
                app.remove_search_results_for_paths(&selection);
            }
            crate::infra::trash::delete_items(
                selection,
                app.active_item_path.clone(),
                sender.clone(),
            );
        }
        AppMsg::FileOps(FileOpsMsg::EmptyTrash) => {
            app.folder_cache
                .remove(&app.cache_key(&std::path::PathBuf::from(crate::ui::constants::TRASH_URI)));
            app.handle_empty_trash(sender);
        }
        AppMsg::FileOps(FileOpsMsg::RestoreItem(path)) => {
            if let Some(parent) = path.parent() {
                app.folder_cache.remove(&app.cache_key(parent));
            }
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
        }
        AppMsg::FileOps(FileOpsMsg::PromptNewFolder) => app.show_prompt_new_folder(sender),
        AppMsg::FileOps(FileOpsMsg::PromptNewFile) => app.show_prompt_new_file(sender),
        AppMsg::Properties(PropertiesMsg::OpenFileProperties(path)) => {
            app.handle_open_file_properties(path, sender)
        }
        AppMsg::Properties(PropertiesMsg::ToggleFilePropertiesPanel) => {
            app.toggle_sidebar_right_panel(crate::domain::model::RightPanelType::Properties, sender)
        }
        AppMsg::Properties(PropertiesMsg::FilePropertiesReady {
            session,
            sections,
            open_with,
        }) => app.handle_file_properties_ready(session, sections, open_with),
        AppMsg::Properties(PropertiesMsg::CopyPropertyValue(text)) => {
            app.handle_copy_property_value(text, sender)
        }
        AppMsg::Properties(PropertiesMsg::OpenPropertiesTargetWith) => {
            app.handle_open_properties_target_with(sender)
        }
        AppMsg::FileOps(FileOpsMsg::SetDisableDragAndDrop(val)) => {
            app.handle_set_disable_drag_and_drop(val)
        }
        AppMsg::FileOps(FileOpsMsg::HandleDrop {
            source_paths,
            dest_path,
        }) => app.handle_drop_items(source_paths, dest_path, sender),
        AppMsg::FileOps(FileOpsMsg::HandleExternalDrop {
            source_paths,
            dest_path,
        }) => app.handle_external_drop_items(source_paths, dest_path, sender),

        AppMsg::Undo(UndoMsg::Undo) => app.handle_undo(sender),
        AppMsg::Undo(UndoMsg::Redo) => app.handle_redo(sender),
        AppMsg::FileOps(FileOpsMsg::TrashSucceeded(items)) => {
            let paths: Vec<std::path::PathBuf> = items.iter().map(|i| i.path.clone()).collect();
            app.purge_paths_from_all_tabs(&paths);
            app.record_trash(items, sender);
        }
        AppMsg::FileOps(FileOpsMsg::MoveSucceeded { items, dest_dir }) => {
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.file_op_history
                .push_undo(crate::ui::undo_redo::FileOp::Move { items, dest_dir });
        }
        AppMsg::FileOps(FileOpsMsg::CopySucceeded { copies, dest_dir }) => {
            let copy_dests = copies.into_iter().map(|(_, dest)| dest).collect();
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.file_op_history
                .push_undo(crate::ui::undo_redo::FileOp::Copy {
                    copies: copy_dests,
                    dest_dir,
                });
        }
        AppMsg::Undo(UndoMsg::UndoMoveComplete {
            redo_items,
            dest_dir,
        }) => {
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.handle_undo_move_complete(redo_items, dest_dir, sender);
        }
        AppMsg::Undo(UndoMsg::UndoMoveFailed(op)) => app.handle_undo_move_failed(op),
        AppMsg::Undo(UndoMsg::UndoTrashComplete { items }) => {
            for item in &items {
                if let Some(parent) = item.path.parent() {
                    app.folder_cache.remove(&app.cache_key(parent));
                }
            }
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            app.handle_undo_trash_complete(items, sender);
        }
        AppMsg::Undo(UndoMsg::UndoTrashFailed(op)) => app.handle_undo_trash_failed(op),
        AppMsg::Undo(UndoMsg::RedoMoveComplete { items, dest_dir }) => {
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            app.handle_redo_move_complete(items, dest_dir, sender);
        }
        AppMsg::Undo(UndoMsg::RedoMoveFailed(op)) => app.handle_redo_move_failed(op),
        AppMsg::Undo(UndoMsg::RedoTrashComplete { items }) => {
            for item in &items {
                if let Some(parent) = item.path.parent() {
                    app.folder_cache.remove(&app.cache_key(parent));
                }
            }
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            app.handle_redo_trash_complete(items, sender);
        }
        AppMsg::Undo(UndoMsg::RedoTrashFailed(op)) => app.handle_redo_trash_failed(op),

        AppMsg::FileOps(FileOpsMsg::FileDeleted(path)) => app.handle_file_deleted_dispatch(path),
        AppMsg::FileOps(FileOpsMsg::FileChanged(path)) => {
            app.handle_file_changed_dispatch(path, sender)
        }
        AppMsg::FileOps(FileOpsMsg::StartRename(path)) => app.handle_start_rename(path),
        AppMsg::FileOps(FileOpsMsg::TriggerRenameSelection) => {
            app.handle_trigger_rename_selection(sender)
        }
        AppMsg::FileOps(FileOpsMsg::ItemMoved { old_path, new_path }) => {
            app.handle_item_moved(old_path, new_path, sender)
        }
        AppMsg::QuickList(QuickListMsg::PerformQuickTransfer { dest, is_cut }) => {
            app.handle_perform_quick_transfer(dest, is_cut, sender)
        }
        other => return Err(other),
    }
    Ok(())
}
