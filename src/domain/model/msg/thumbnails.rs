//! Thumbnail and video-preview generation.

use gtk::gdk;
use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ThumbnailsMsg {
    /// Notifies that the vertical viewport changed with normalized scroll progress values.
    UpdateVisibleThumbnailsViewport {
        progress_top: f64,
        progress_bottom: f64,
    },
    /// Toggles automatic muted video playback on card selection.
    SetAutoplayVideoPreviews(bool),
    /// Triggers deferred inline video playback after a debounce timeout.
    TriggerVideoPreview(PathBuf),
    /// Updates the target pixel dimension for rendered and cached thumbnails.
    SetThumbnailSize(i32),
    /// Set the thread count used by each FFmpeg thumbnail process.
    SetFfmpegThreads(usize),
    /// Set the seek offset (in seconds) for video thumbnail frame grabs.
    SetFfmpegSeekSeconds(f64),
    /// Toggle automatic rotation based on video container metadata.
    SetFfmpegAutoRotate(bool),
    /// Set the maximum concurrent thumbnail rendering tasks.
    SetThumbnailThreads(usize),
    /// Toggles the lazy thumbnail generation setting.
    SetLazyThumbnails(bool),
    /// Toggles storing thumbnails inside zip archives.
    SetArchiveThumbnailCache(bool),
    /// Check for visible thumbnails in the current viewport and request generation for any missing ones
    CheckVisibleThumbnails,
    /// A custom file-icon image was decoded on a worker thread.
    FileIconTextureReady {
        path: PathBuf,
        texture: gtk::gdk::Texture,
    },
    ThumbnailReady {
        grid_idx: u32,
        texture: gdk::Texture,
        load_id: u64,
        tab_index: usize,
    },
    /// Requests on-demand thumbnail generation for a single visible item.
    RequestThumbnail {
        grid_idx: u32,
        path: std::path::PathBuf,
        load_id: u64,
    },
    /// Toggles the global thumbnail generation setting.
    ///
    /// When disabled, no thumbnails are generated for any file type, improving
    /// performance on low-spec systems or for users who prefer a minimal view.
    SetShowThumbnails(bool),
    /// Toggles thumbnail generation for a specific file type.
    ///
    /// # Fields
    /// - `type_name`: One of "images", "videos", "fonts", or "pdfs"
    /// - `enabled`: Whether thumbnails should be generated for this type
    SetThumbnailType { type_name: String, enabled: bool },
}
