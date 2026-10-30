//! Folder loading and cache.

use crate::domain::model::FileLoadContext;
use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum LoadingMsg {
    /// Activates the spinner after the debounce timeout if the session is still current.
    ShowLoadingSpinner(u64),
    /// Evicts the current directory from the folder cache and navigates to the given path.
    InvalidateCacheAndNavigate(PathBuf),
    /// Set the batch size for streaming directory entries into the UI grid.
    SetLoaderBatchSize(usize),
    /// Set the capacity of the in-memory folder cache.
    SetFolderCacheCapacity(usize),
    /// Delivers an incremental batch of load contexts to the grid view.
    FolderLoadedChunk {
        load_id: u64,
        chunk: Vec<FileLoadContext>,
        is_cached: bool,
    },
    /// Finalizes directory loading after all chunks are appended.
    FolderLoadedFinish { load_id: u64 },
    /// Carries the result of an async, off-thread folder enumeration back to
    /// the main loop so the grid can be populated without blocking the GTK thread.
    FolderLoaded {
        /// The path that was enumerated, used to guard against stale navigations.
        path: PathBuf,
        /// The session ID from `load_id` at the time navigation was initiated.
        /// Results whose `load_id` differs from the current counter are discarded.
        load_id: u64,
        /// Fully processed, filtered, and sorted file contexts ready for the grid.
        items: Vec<FileLoadContext>,
        /// Visual-media paths collected for thumbnail dispatch.
        media_tasks: Vec<(u32, PathBuf)>,
        /// Whether the folder opens in list mode. `true` when the item count
        /// exceeds `auto_list_threshold`, otherwise the current session mode.
        list_mode: bool,
    },
}
