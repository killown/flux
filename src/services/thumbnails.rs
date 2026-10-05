use crate::model::{AppMsg, FluxApp};
use crate::utils;
use futures::stream::{self, StreamExt};
use relm4::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Semaphore;

/// Clamp the thumbnail worker count so it can't eat the whole FD budget.
///
/// Each thumbnail holds a couple of fds while it runs, and on a folder with
/// thousands of images that adds up fast. Give thumbnails a quarter of
/// RLIMIT_NOFILE and leave the rest for GTK, GIO, SQLite and the terminal.
fn effective_thumbnail_permits(configured: usize) -> usize {
    let mut lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let soft = unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) == 0 {
            lim.rlim_cur as usize
        } else {
            1024
        }
    };

    // Below 2 the pipeline just serializes for no reason.
    let budget = (soft / 4).max(2);
    configured.max(1).min(budget)
}

/// Tracks active lazy thumbnail generation tasks to allow viewport-based cancellation.
#[derive(Default, Debug)]
pub struct ThumbnailTaskManager {
    active_tokens: Mutex<HashMap<u32, Arc<AtomicBool>>>,
    semaphore: OnceLock<Arc<Semaphore>>,
}

impl ThumbnailTaskManager {
    /// Returns the shared semaphore matching the configured worker thread limit,
    /// clamped to the process's FD budget.
    pub fn get_semaphore(&self, max_threads: usize) -> Arc<Semaphore> {
        let permits = effective_thumbnail_permits(max_threads);
        self.semaphore
            .get_or_init(|| Arc::new(Semaphore::new(permits)))
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
        tab_index: usize,
        sender: AsyncComponentSender<Self>,
    ) {
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

                    async move {
                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
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

                        let texture = utils::get_or_create_thumbnail(&media_path).await;

                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        if let Some(texture) = texture {
                            let _ = inner_sender.input_sender().send(AppMsg::ThumbnailReady {
                                grid_idx,
                                texture,
                                load_id: session_id,
                                tab_index,
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

            let texture = utils::get_or_create_thumbnail(&media_path).await;

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
