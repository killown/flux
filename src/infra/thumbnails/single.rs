use super::manager::ThumbnailTaskManager;
use crate::domain::model::Image;
use crate::utils;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Inputs for [`spawn_single`].
pub struct SingleRequest {
    /// Grid index the thumbnail is for, echoed back to `on_ready`.
    pub grid_idx: u32,
    /// File to thumbnail.
    pub media_path: PathBuf,
    /// Navigation session this request belongs to.
    pub expected_session: u64,
    /// Live navigation session counter. A mismatch with `expected_session`
    /// means the request is stale and its result is discarded.
    pub session: Arc<AtomicU64>,
    /// Set when the item scrolled out of the viewport.
    pub cancel_flag: Arc<AtomicBool>,
    /// Shared semaphore bounding concurrent thumbnail work.
    pub semaphore: Arc<Semaphore>,
    /// Tracker whose entry for `grid_idx` is cleared when the task ends.
    pub manager: Arc<ThumbnailTaskManager>,
}

/// Spawns a background task that generates (or cache-hits) the thumbnail for a
/// single grid item on demand and hands the result to `on_ready`.
///
/// Used exclusively by the lazy-thumbnail path.
///
/// * A stale result from a superseded navigation session is silently discarded.
/// * XDG FreeDesktop thumbnail caches (`~/.cache/thumbnails/`) are consulted
///   first by `utils::get_or_create_thumbnail`, keeping cache hits fast.
/// * Concurrency is bounded: the task acquires a permit from the shared
///   semaphore before performing any I/O or spawning external tools.
/// * Rapid scrolling cancels out-of-viewport tasks before they acquire the
///   permit or render.
///
/// `on_ready` receives the grid index and the pixels once the task finished
/// without being cancelled or superseded.
pub fn spawn_single<F>(request: SingleRequest, on_ready: F)
where
    F: FnOnce(u32, Image) + Send + 'static,
{
    crate::hit!("spawn_single_thumbnail");
    let SingleRequest {
        grid_idx,
        media_path,
        expected_session,
        session,
        cancel_flag,
        semaphore,
        manager,
    } = request;

    relm4::spawn(async move {
        let stale = || {
            cancel_flag.load(Ordering::Acquire)
                || session.load(Ordering::Acquire) != expected_session
        };

        if stale() {
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

        if stale() {
            manager.complete_task(grid_idx);
            return;
        }

        let texture = utils::media::get_or_create_thumbnail(&media_path).await;

        if stale() {
            manager.complete_task(grid_idx);
            return;
        }

        manager.complete_task(grid_idx);

        if let Some(texture) = texture {
            on_ready(grid_idx, utils::media::image_from_texture(&texture));
        }
    });
}
