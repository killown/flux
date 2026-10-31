use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, ArchiveMsg, LoadingMsg};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
    match msg {
        AppMsg::Loading(LoadingMsg::ShowLoadingSpinner(session)) => {
            app.handle_show_loading_spinner(session)
        }
        AppMsg::Loading(LoadingMsg::FolderLoadedChunk {
            load_id,
            chunk,
            is_cached,
        }) => app.handle_folder_loaded_chunk(load_id, chunk, is_cached, sender),
        AppMsg::Loading(LoadingMsg::FolderLoadedFinish { load_id }) => {
            app.handle_folder_loaded_finish(load_id)
        }
        AppMsg::Loading(LoadingMsg::FolderLoaded {
            path,
            load_id,
            items,
            media_tasks,
            list_mode,
        }) => app.handle_folder_loaded_begin(path, load_id, items, media_tasks, list_mode, sender),
        AppMsg::Loading(LoadingMsg::InvalidateCacheAndNavigate(path)) => {
            app.handle_invalidate_cache_and_navigate(path, sender)
        }
        AppMsg::Loading(LoadingMsg::SetFolderCacheCapacity(val)) => {
            app.handle_set_folder_cache_capacity(val)
        }
        AppMsg::Loading(LoadingMsg::SetLoaderBatchSize(val)) => {
            app.handle_set_loader_batch_size(val)
        }
        AppMsg::Archive(ArchiveMsg::PromptArchiveDeletion {
            archive_path,
            inner_path,
        }) => {
            app.show_archive_deletion_warning(archive_path, inner_path, sender);
        }
        AppMsg::Archive(ArchiveMsg::ExtractArchive) => app.handle_extract_archive(sender),
        AppMsg::Archive(ArchiveMsg::PromptNestedArchiveExtract {
            archive_path,
            inner_path,
        }) => app.show_nested_archive_prompt(archive_path, inner_path, sender),
        AppMsg::Archive(ArchiveMsg::ExtractNestedArchive {
            archive_path,
            inner_path,
        }) => app.handle_extract_nested_archive(archive_path, inner_path, sender),
        AppMsg::Archive(ArchiveMsg::ArchiveLocksProbed { load_id, encrypted }) => {
            app.handle_archive_locks_probed(load_id, encrypted)
        }
        AppMsg::Archive(ArchiveMsg::ArchiveLoaded {
            archive_path,
            prefix,
            password,
            load_id,
            result,
        }) => app.handle_archive_loaded(archive_path, prefix, password, load_id, result, sender),
        AppMsg::Archive(ArchiveMsg::PromptArchivePassword {
            archive_path,
            prefix,
            wrong_password,
        }) => app.show_prompt_archive_password(archive_path, prefix, wrong_password, sender),
        AppMsg::Archive(ArchiveMsg::SaveArchivePasswordToKeyring {
            archive_path,
            password,
        }) => app.queue_archive_password_save(archive_path, password),
        AppMsg::Archive(ArchiveMsg::ForgetArchivePassword(target)) => {
            app.handle_forget_archive_password(target, sender)
        }
        other => return Err(other),
    }
    Ok(())
}
