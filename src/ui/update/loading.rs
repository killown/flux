use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::ShowLoadingSpinner(session) => app.handle_show_loading_spinner(session),
        AppMsg::FolderLoadedChunk {
            load_id,
            chunk,
            is_cached,
        } => app.handle_folder_loaded_chunk(load_id, chunk, is_cached, sender),
        AppMsg::FolderLoadedFinish { load_id } => app.handle_folder_loaded_finish(load_id),
        AppMsg::FolderLoaded {
            path,
            load_id,
            items,
            media_tasks,
        } => app.handle_folder_loaded_begin(path, load_id, items, media_tasks, sender),
        AppMsg::InvalidateCacheAndNavigate(path) => {
            app.handle_invalidate_cache_and_navigate(path, sender)
        }
        AppMsg::SetFolderCacheCapacity(val) => app.handle_set_folder_cache_capacity(val),
        AppMsg::SetLoaderBatchSize(val) => app.handle_set_loader_batch_size(val),
        AppMsg::PromptArchiveDeletion {
            archive_path,
            inner_path,
        } => {
            app.show_archive_deletion_warning(archive_path, inner_path, sender);
        }
        AppMsg::ExtractArchive => app.handle_extract_archive(sender),
        AppMsg::ArchiveLoaded {
            archive_path,
            prefix,
            password,
            load_id,
            result,
        } => app.handle_archive_loaded(archive_path, prefix, password, load_id, result, sender),
        AppMsg::PromptArchivePassword {
            archive_path,
            prefix,
            wrong_password,
        } => app.show_prompt_archive_password(archive_path, prefix, wrong_password, sender),
        other => return Err(other),
    }
    Ok(())
}
