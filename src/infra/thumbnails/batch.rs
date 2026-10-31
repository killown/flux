use crate::domain::model::Image;
use crate::utils;
use futures::stream::{self, StreamExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Inputs for [`spawn_batch`].
pub struct BatchRequest {
    /// `(grid_idx, path)` pairs to thumbnail.
    pub tasks: Vec<(u32, PathBuf)>,
    /// Navigation session this batch belongs to.
    pub expected_session: u64,
    /// Live navigation session counter. A mismatch with `expected_session`
    /// means the batch is stale and the remaining work is skipped.
    pub session: Arc<AtomicU64>,
    /// Shared semaphore bounding concurrent thumbnail work, clamped to the
    /// process's FD budget.
    pub semaphore: Arc<Semaphore>,
    /// How many futures are polled at once.
    pub max_concurrency: usize,
    /// When set, each task first checks off the GTK thread that no download
    /// scratch file (`.part`, `.crdownload`, ...) sits next to the target, and
    /// skips it if one does. The check scans the whole parent directory.
    pub skip_active_downloads: bool,
}

/// Spawns a background task that thumbnails every entry of `request.tasks` with
/// bounded concurrency and hands each result to `on_ready` as `(grid_idx, image)`.
///
/// Results for a superseded navigation session are discarded. `on_ready` is
/// called from worker tasks, one clone per task.
pub fn spawn_batch<F>(request: BatchRequest, on_ready: F)
where
    F: Fn(u32, Image) + Clone + Send + 'static,
{
    crate::hit!("spawn_thumbnail_loader");
    let BatchRequest {
        tasks,
        expected_session,
        session,
        semaphore,
        max_concurrency,
        skip_active_downloads,
    } = request;

    relm4::spawn(async move {
        // Process tasks with bounded concurrency to eliminate task churn.
        // The semaphore is the real FD throttle, buffer_unordered only
        // limits how many futures are polled at once.
        stream::iter(tasks)
            .map(move |(grid_idx, media_path)| {
                let session = session.clone();
                let sem = semaphore.clone();
                let on_ready = on_ready.clone();

                async move {
                    if session.load(Ordering::Acquire) != expected_session {
                        return;
                    }

                    // A scratch sibling means the download is still in
                    // progress and the file is not safe to thumbnail yet.
                    // The scan reads the whole parent directory, so it runs
                    // on the blocking pool, before any FD permit is taken.
                    if skip_active_downloads {
                        let probe = media_path.clone();
                        let downloading = tokio::task::spawn_blocking(move || {
                            crate::infra::downloads::has_active_download_sibling(&probe)
                        })
                        .await
                        .unwrap_or(false);

                        if downloading || session.load(Ordering::Acquire) != expected_session {
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

                    if session.load(Ordering::Acquire) != expected_session {
                        return;
                    }

                    let texture = utils::media::get_or_create_thumbnail(&media_path).await;

                    if session.load(Ordering::Acquire) != expected_session {
                        return;
                    }

                    if let Some(texture) = texture {
                        on_ready(grid_idx, utils::media::image_from_texture(&texture));
                    }
                }
            })
            .buffer_unordered(max_concurrency)
            .collect::<Vec<()>>()
            .await;
    });
}
