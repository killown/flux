use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::FileIconTextureReady { path, texture } => {
            crate::hit!("handle_file_icon_texture_ready");
            crate::utils::icon::insert_cached_file_icon_texture(&path, texture);
            crate::utils::icon::end_file_icon_decode(&path);
            let files = &mut app.tabs[app.active_tab_index].files;
            for i in 0..files.len() {
                let uses_icon = files.get(i).is_some_and(|w| {
                    w.borrow()
                        .icon
                        .downcast_ref::<gtk::gio::FileIcon>()
                        .and_then(|fi| fi.file().path())
                        .is_some_and(|icon_path| icon_path == path)
                });
                if uses_icon {
                    if let Some(w) = files.get(i) {
                        crate::hit!("rebind_grid_item");
                        let item = w.borrow().clone();
                        files.remove(i);
                        files.insert(i, item);
                    }
                }
            }
        }
        AppMsg::UpdateVisibleThumbnailsViewport {
            progress_top,
            progress_bottom,
        } => app.handle_update_visible_thumbnails_viewport(progress_top, progress_bottom, sender),
        AppMsg::TriggerVideoPreview(path) => app.handle_trigger_video_preview(path),
        AppMsg::SetAutoplayVideoPreviews(val) => app.handle_set_autoplay_video_previews(val),
        AppMsg::SetThumbnailSize(val) => app.handle_set_thumbnail_size(val, sender),
        AppMsg::SetShowThumbnails(val) => app.handle_set_show_thumbnails(val, sender),
        AppMsg::SetLazyThumbnails(val) => app.handle_set_lazy_thumbnails(val),
        AppMsg::SetArchiveThumbnailCache(val) => app.handle_set_archive_thumbnail_cache(val),
        AppMsg::SetThumbnailThreads(val) => app.handle_set_thumbnail_threads(val),
        AppMsg::SetThumbnailType { type_name, enabled } => {
            app.handle_set_thumbnail_type(type_name, enabled, sender)
        }
        AppMsg::CheckVisibleThumbnails => app.check_visible_thumbnails(sender),
        AppMsg::RequestThumbnail { grid_idx, path, .. } => {
            app.handle_request_thumbnail(grid_idx, path, sender)
        }
        AppMsg::ThumbnailReady {
            grid_idx,
            texture,
            load_id,
            tab_index,
        } => app.handle_thumbnail_ready(grid_idx, texture, load_id, tab_index),
        AppMsg::SetFfmpegThreads(val) => app.handle_set_ffmpeg_threads(val),
        AppMsg::SetFfmpegSeekSeconds(val) => app.handle_set_ffmpeg_seek_seconds(val),
        AppMsg::SetFfmpegAutoRotate(val) => app.handle_set_ffmpeg_auto_rotate(val),
        other => return Err(other),
    }
    Ok(())
}
