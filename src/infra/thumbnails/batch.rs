use crate::domain::model::{AppMsg, ThumbnailsMsg, FluxApp};
use crate::utils;
use futures::stream::{self, StreamExt};
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
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
        crate::hit!("spawn_thumbnail_loader");
        let session_arc = self.load_id.clone();

        if media_tasks.is_empty() {
            return;
        }

        let max_threads = self.config.ui.thumbnail_threads.max(1);
        // Clamp against RLIMIT_NOFILE so eager cold loads can't blow the FD
        // ceiling on folders with thousands of images.
        let sem = self.thumbnail_manager.get_semaphore(max_threads);

        relm4::spawn(async move {
            // Process tasks with bounded concurrency to eliminate task churn.
            // The semaphore is the real FD throttle, buffer_unordered only
            // limits how many futures are polled at once.
            stream::iter(media_tasks)
                .map(|(grid_idx, media_path)| {
                    let inner_sender = sender.clone();
                    let inner_session = session_arc.clone();
                    let session_id = current_session;
                    let sem = sem.clone();
                    let check_downloads = skip_active_downloads;

                    async move {
                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        // A scratch sibling means the download is still in
                        // progress and the file is not safe to thumbnail yet.
                        // The scan reads the whole parent directory, so it runs
                        // on the blocking pool, before any FD permit is taken.
                        if check_downloads {
                            let probe = media_path.clone();
                            let downloading = tokio::task::spawn_blocking(move || {
                                crate::app::update::watcher::has_active_download_sibling(&probe)
                            })
                            .await
                            .unwrap_or(false);

                            if downloading || inner_session.load(Ordering::Acquire) != session_id {
                                return;
                            }
                        }

                        // Acquire a permit before opening any file. A stale
                        // task will still block here, but the session check
                        // right after bails out before any I/O happens, so
                        // it doesn't waste an FD budget slot on real work.
                        let _permit = match sem.acquire().await {
                            Ok(p) => p,
                            Err(_) => return,
                        };

                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        let texture = utils::media::get_or_create_thumbnail(&media_path).await;

                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        if let Some(texture) = texture {
                            let _ = inner_sender.input_sender().send(AppMsg::Thumbnails(ThumbnailsMsg::ThumbnailReady {
                                grid_idx,
                                texture,
                                load_id: session_id,
                                tab_index,
                            }));
                        }
                    }
                })
                .buffer_unordered(max_threads)
                .collect::<Vec<()>>()
                .await;
        });
    }
}
