use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, ThumbnailsMsg};
use crate::infra::thumbnails::{spawn_batch, spawn_single, BatchRequest, SingleRequest};
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Semaphore;

impl FluxApp {
    /// Spawns the eager thumbnail pipeline for `media_tasks` and reports each
    /// finished thumbnail as [`ThumbnailsMsg::ThumbnailReady`].
    pub fn spawn_thumbnail_loader(
        &self,
        media_tasks: Vec<(u32, PathBuf)>,
        current_session: u64,
        tab_index: usize,
        sender: AsyncComponentSender<Self>,
    ) {
        self.spawn_thumbnail_loader_inner(media_tasks, current_session, tab_index, sender, false);
    }

    /// Same as [`Self::spawn_thumbnail_loader`], but each task first checks,
    /// off the GTK thread, that no download scratch file (`.part`,
    /// `.crdownload`, ...) sits next to the target, and skips it if one does.
    ///
    /// Used by the file watcher: the check scans the whole parent directory,
    /// which must not run inside the event handler.
    pub fn spawn_thumbnail_loader_unless_downloading(
        &self,
        media_tasks: Vec<(u32, PathBuf)>,
        current_session: u64,
        tab_index: usize,
        sender: AsyncComponentSender<Self>,
    ) {
        self.spawn_thumbnail_loader_inner(media_tasks, current_session, tab_index, sender, true);
    }

    fn spawn_thumbnail_loader_inner(
        &self,
        media_tasks: Vec<(u32, PathBuf)>,
        current_session: u64,
        tab_index: usize,
        sender: AsyncComponentSender<Self>,
        skip_active_downloads: bool,
    ) {
        if media_tasks.is_empty() {
            return;
        }

        let max_threads = self.config.ui.thumbnail_threads.max(1);
        // Clamp against RLIMIT_NOFILE so eager cold loads can't blow the FD
        // ceiling on folders with thousands of images.
        let semaphore = self.thumbnail_manager.get_semaphore(max_threads);

        spawn_batch(
            BatchRequest {
                tasks: media_tasks,
                expected_session: current_session,
                session: self.load_id.clone(),
                semaphore,
                max_concurrency: max_threads,
                skip_active_downloads,
            },
            move |grid_idx, image| {
                let _ =
                    sender
                        .input_sender()
                        .send(AppMsg::Thumbnails(ThumbnailsMsg::ThumbnailReady {
                            grid_idx,
                            image,
                            load_id: current_session,
                            tab_index,
                        }));
            },
        );
    }

    /// Spawns a worker that generates (or cache-hits) the thumbnail for a
    /// single grid item on demand and reports it as
    /// [`ThumbnailsMsg::ThumbnailReady`].
    ///
    /// Used exclusively by the lazy-thumbnail path (`config.ui.lazy_thumbnails = true`).
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_single_thumbnail(
        &self,
        grid_idx: u32,
        media_path: PathBuf,
        current_session: u64,
        tab_index: usize,
        cancel_flag: Arc<AtomicBool>,
        semaphore: Arc<Semaphore>,
        sender: AsyncComponentSender<Self>,
    ) {
        spawn_single(
            SingleRequest {
                grid_idx,
                media_path,
                expected_session: current_session,
                session: self.load_id.clone(),
                cancel_flag,
                semaphore,
                manager: self.thumbnail_manager.clone(),
            },
            move |grid_idx, image| {
                let _ =
                    sender
                        .input_sender()
                        .send(AppMsg::Thumbnails(ThumbnailsMsg::ThumbnailReady {
                            grid_idx,
                            image,
                            load_id: current_session,
                            tab_index,
                        }));
            },
        );
    }
}
