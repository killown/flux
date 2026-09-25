use crate::model::{AppMsg, FluxApp};
use crate::utils;
use futures::stream::{self, StreamExt};
use relm4::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Semaphore;

/// Tracks active lazy thumbnail generation tasks to allow viewport-based cancellation.
#[derive(Default, Debug)]
pub struct ThumbnailTaskManager {
    active_tokens: Mutex<HashMap<u32, Arc<AtomicBool>>>,
    semaphore: OnceLock<Arc<Semaphore>>,
}

impl ThumbnailTaskManager {
    /// Returns the shared semaphore matching the configured worker thread limit.
    pub fn get_semaphore(&self, max_threads: usize) -> Arc<Semaphore> {
        self.semaphore
            .get_or_init(|| Arc::new(Semaphore::new(max_threads.max(1))))
            .clone()
    }

    /// Cancels tasks for grid indices that are no longer within the visible viewport bounds.
    /// Returns the list of cancelled indices so caller can evict them from pending tracking.
    pub fn cancel_out_of_viewport(&self, visible_start: u32, visible_end: u32) -> Vec<u32> {
        let mut cancelled = Vec::new();
        let mut tokens = self.active_tokens.lock().unwrap();
        tokens.retain(|&idx, cancel_flag| {
            if idx < visible_start || idx > visible_end {
                cancel_flag.store(true, Ordering::Release);
                cancelled.push(idx);
                false
            } else {
                true
            }
        });
        cancelled
    }

    /// Registers a cancellation flag for a newly requested thumbnail index.
    pub fn register_token(&self, grid_idx: u32) -> Arc<AtomicBool> {
        let mut tokens = self.active_tokens.lock().unwrap();
        if let Some(old) = tokens.remove(&grid_idx) {
            old.store(true, Ordering::Release);
        }
        let flag = Arc::new(AtomicBool::new(false));
        tokens.insert(grid_idx, flag.clone());
        flag
    }

    /// Clears task tracking on completion.
    pub fn complete_task(&self, grid_idx: u32) {
        if let Ok(mut tokens) = self.active_tokens.lock() {
            tokens.remove(&grid_idx);
        }
    }

    /// Cancels all ongoing tasks (e.g. during directory changes).
    pub fn clear_and_cancel_all(&self) {
        if let Ok(mut tokens) = self.active_tokens.lock() {
            for (_, flag) in tokens.drain() {
                flag.store(true, Ordering::Release);
            }
        }
    }
}

impl FluxApp {
    pub fn spawn_thumbnail_loader(
        &self,
        media_tasks: Vec<(u32, PathBuf)>,
        current_session: u64,
        sender: AsyncComponentSender<Self>,
    ) {
        let session_arc = self.load_id.clone();

        if media_tasks.is_empty() {
            return;
        }

        let max_threads = self.config.ui.thumbnail_threads.max(1);

        relm4::spawn(async move {
            // Process tasks with bounded concurrency to eliminate task churn
            stream::iter(media_tasks)
                .map(|(grid_idx, media_path)| {
                    let inner_sender = sender.clone();
                    let inner_session = session_arc.clone();
                    let session_id = current_session;

                    async move {
                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        let texture = utils::get_or_create_thumbnail(&media_path).await;

                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        if let Some(texture) = texture {
                            inner_sender.input(AppMsg::ThumbnailReady {
                                grid_idx,
                                texture,
                                load_id: session_id,
                            });
                        }
                    }
                })
                .buffer_unordered(max_threads)
                .collect::<Vec<()>>()
                .await;
        });
    }

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
    pub fn spawn_single_thumbnail(
        &self,
        grid_idx: u32,
        media_path: PathBuf,
        current_session: u64,
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

            let texture = utils::get_or_create_thumbnail(&media_path).await;

            if cancel_flag.load(Ordering::Acquire)
                || session_arc.load(Ordering::Acquire) != current_session
            {
                manager.complete_task(grid_idx);
                return;
            }

            manager.complete_task(grid_idx);

            if let Some(texture) = texture {
                sender.input(AppMsg::ThumbnailReady {
                    grid_idx,
                    texture,
                    load_id: current_session,
                });
            }
        });
    }
}
