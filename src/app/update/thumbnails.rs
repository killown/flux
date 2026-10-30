use crate::domain::model::{AppMsg, FluxApp, ThumbnailsMsg};
use adw::prelude::*;
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::Thumbnails(ThumbnailsMsg::FileIconTextureReady { path, texture }) => {
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
        AppMsg::Thumbnails(ThumbnailsMsg::UpdateVisibleThumbnailsViewport {
            progress_top,
            progress_bottom,
        }) => app.handle_update_visible_thumbnails_viewport(progress_top, progress_bottom, sender),
        AppMsg::Thumbnails(ThumbnailsMsg::TriggerVideoPreview(path)) => {
            app.handle_trigger_video_preview(path)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetAutoplayVideoPreviews(val)) => {
            app.handle_set_autoplay_video_previews(val)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetThumbnailSize(val)) => {
            app.handle_set_thumbnail_size(val, sender)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetShowThumbnails(val)) => {
            app.handle_set_show_thumbnails(val, sender)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetLazyThumbnails(val)) => {
            app.handle_set_lazy_thumbnails(val)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetArchiveThumbnailCache(val)) => {
            app.handle_set_archive_thumbnail_cache(val)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetThumbnailThreads(val)) => {
            app.handle_set_thumbnail_threads(val)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetThumbnailType { type_name, enabled }) => {
            app.handle_set_thumbnail_type(type_name, enabled, sender)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::CheckVisibleThumbnails) => {
            app.check_visible_thumbnails(sender)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::RequestThumbnail { grid_idx, path, .. }) => {
            app.handle_request_thumbnail(grid_idx, path, sender)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::ThumbnailReady {
            grid_idx,
            texture,
            load_id,
            tab_index,
        }) => app.handle_thumbnail_ready(grid_idx, texture, load_id, tab_index),
        AppMsg::Thumbnails(ThumbnailsMsg::SetFfmpegThreads(val)) => {
            app.handle_set_ffmpeg_threads(val)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetFfmpegSeekSeconds(val)) => {
            app.handle_set_ffmpeg_seek_seconds(val)
        }
        AppMsg::Thumbnails(ThumbnailsMsg::SetFfmpegAutoRotate(val)) => {
            app.handle_set_ffmpeg_auto_rotate(val)
        }
        other => return Err(other),
    }
    Ok(())
}
