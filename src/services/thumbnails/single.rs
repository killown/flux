use crate::model::{AppMsg, FluxApp};
use crate::utils;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

impl FluxApp {
    /// Spawns a background worker to generate (or cache-hit) the thumbnail for a
    /// single grid item on demand.
    ///
    /// Used exclusively by the lazy-thumbnail path (`config.ui.lazy_thumbnails = true`).
    /// Mirrors the session-invalidation and XDG-cache semantics of
    /// [`Self::spawn_thumbnail_loader`] so that:
    ///
    /// * A stale result from a superseded navigation session is silently discarded.
    /// * XDG FreeDesktop thumbnail caches (`~/.cache/thumbnails/`) are consulted
    ///   first by `utils::get_or_create_thumbnail`, keeping cache hits fast.
    /// * Concurrency is naturally bounded: tasks acquire a permit from a shared
    ///   `tokio::sync::Semaphore` before performing any I/O or spawning external tools.
    /// * Rapid scrolling cancels out-of-viewport tasks before acquiring the permit or rendering.
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
        let session_arc = self.load_id.clone();
        let manager = self.thumbnail_manager.clone();

        relm4::spawn(async move {
            if cancel_flag.load(Ordering::Acquire)
                || session_arc.load(Ordering::Acquire) != current_session
            {
                manager.complete_task(grid_idx);
                return;
            }

            let _permit = match semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => {
                    manager.complete_task(grid_idx);
                    return;
                }
            };

            if cancel_flag.load(Ordering::Acquire)
                || session_arc.load(Ordering::Acquire) != current_session
            {
                manager.complete_task(grid_idx);
                return;
            }

            let texture = utils::media::get_or_create_thumbnail(&media_path).await;

            if cancel_flag.load(Ordering::Acquire)
                || session_arc.load(Ordering::Acquire) != current_session
            {
                manager.complete_task(grid_idx);
                return;
            }

            manager.complete_task(grid_idx);

            if let Some(texture) = texture {
                let _ = sender.input_sender().send(AppMsg::ThumbnailReady {
                    grid_idx,
                    texture,
                    load_id: current_session,
                    tab_index,
                });
            }
        });
    }
}
