use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::CopyPath => app.handle_copy_path(sender),
        AppMsg::CreateSymlink => app.handle_create_link(false, sender),
        AppMsg::CreateHardlink => app.handle_create_link(true, sender),
        AppMsg::Copy => app.handle_copy_or_cut(false, sender),
        AppMsg::Cut => app.handle_copy_or_cut(true, sender),
        AppMsg::Paste => app.handle_paste_from_clipboard(sender),
        AppMsg::PasteImageFromClipboard
        | AppMsg::PasteTextFromClipboard
        | AppMsg::PasteHtmlFromClipboard => {}
        AppMsg::ConfirmReplacePaste {
            files,
            conflicts,
            is_cut,
        } => app.show_confirm_replace_paste(files, conflicts, is_cut, sender),
        AppMsg::PerformPaste { files, is_cut } => app.perform_paste(files, is_cut, sender.clone()),
        AppMsg::PerformPasteForced { files, is_cut } => {
            app.perform_paste_inner(files, is_cut, true, sender.clone())
        }
        AppMsg::PerformRename(old_path, new_name) => {
            app.handle_perform_rename(old_path, new_name, sender)
        }
        AppMsg::Delete => {
            let selection = app.get_selection();
            if app.is_content_searching {
                app.remove_search_results_for_paths(&selection);
            }
            crate::services::trash::delete_items(
                selection,
                app.active_item_path.clone(),
                sender.clone(),
            );
        }
        AppMsg::EmptyTrash => {
            app.folder_cache
                .remove(&app.cache_key(&std::path::PathBuf::from(crate::ui::constants::TRASH_URI)));
            app.handle_empty_trash(sender);
        }
        AppMsg::RestoreItem(path) => {
            if let Some(parent) = path.parent() {
                app.folder_cache.remove(&app.cache_key(parent));
            }
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            sender.input(AppMsg::Refresh);
        }
        AppMsg::PromptNewFolder => app.show_prompt_new_folder(sender),
        AppMsg::PromptNewFile => app.show_prompt_new_file(sender),
        AppMsg::OpenFileProperties(path) => {
            let properties_win = crate::ui::FileProperties::builder().launch(path).detach();
            properties_win.widget().present();
        }
        AppMsg::SetDisableDragAndDrop(val) => app.handle_set_disable_drag_and_drop(val),
        AppMsg::HandleDrop {
            source_paths,
            dest_path,
        } => app.handle_drop_items(source_paths, dest_path, sender),
        AppMsg::HandleExternalDrop {
            source_paths,
            dest_path,
        } => app.handle_external_drop_items(source_paths, dest_path, sender),

        AppMsg::Undo => app.handle_undo(sender),
        AppMsg::Redo => app.handle_redo(sender),
        AppMsg::TrashSucceeded(paths) => {
            for p in &paths {
                if let Some(parent) = p.parent() {
                    app.folder_cache.remove(&app.cache_key(parent));
                }
            }
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            app.file_op_history
                .push_undo(crate::ui::undo_redo::FileOp::Trash { paths });
        }
        AppMsg::MoveSucceeded { items, dest_dir } => {
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.file_op_history
                .push_undo(crate::ui::undo_redo::FileOp::Move { items, dest_dir });
        }
        AppMsg::CopySucceeded { copies, dest_dir } => {
            let copy_dests = copies.into_iter().map(|(_, dest)| dest).collect();
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.file_op_history
                .push_undo(crate::ui::undo_redo::FileOp::Copy {
                    copies: copy_dests,
                    dest_dir,
                });
        }
        AppMsg::UndoMoveComplete {
            redo_items,
            dest_dir,
        } => {
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.handle_undo_move_complete(redo_items, dest_dir, sender);
        }
        AppMsg::UndoMoveFailed(op) => app.handle_undo_move_failed(op),
        AppMsg::UndoTrashComplete { paths } => {
            for p in &paths {
                if let Some(parent) = p.parent() {
                    app.folder_cache.remove(&app.cache_key(parent));
                }
            }
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            app.handle_undo_trash_complete(paths, sender);
        }
        AppMsg::UndoTrashFailed(op) => app.handle_undo_trash_failed(op),
        AppMsg::RedoMoveComplete { items, dest_dir } => {
            app.folder_cache.remove(&app.cache_key(&dest_dir));
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            app.handle_redo_move_complete(items, dest_dir, sender);
        }
        AppMsg::RedoMoveFailed(op) => app.handle_redo_move_failed(op),
        AppMsg::RedoTrashComplete { paths } => {
            for p in &paths {
                if let Some(parent) = p.parent() {
                    app.folder_cache.remove(&app.cache_key(parent));
                }
            }
            app.folder_cache.remove(&app.cache_key(&app.current_path));
            app.handle_redo_trash_complete(paths, sender);
        }
        AppMsg::RedoTrashFailed(op) => app.handle_redo_trash_failed(op),

        AppMsg::FileDeleted(path) => app.handle_file_deleted_dispatch(path),
        AppMsg::FileChanged(path) => app.handle_file_changed_dispatch(path, sender),
        AppMsg::StartRename(path) => app.handle_start_rename(path),
        AppMsg::TriggerRenameSelection => app.handle_trigger_rename_selection(sender),
        AppMsg::ItemMoved { old_path, new_path } => {
            app.handle_item_moved(old_path, new_path, sender)
        }
        AppMsg::PerformQuickTransfer { dest, is_cut } => {
            app.handle_perform_quick_transfer(dest, is_cut, sender)
        }
        other => return Err(other),
    }
    Ok(())
}
