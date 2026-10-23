use serde::{Deserialize, Serialize};

/// Per-type thumbnail generation settings for file previews.
///
/// Controls which file types should have visual previews generated in the file grid.
/// Each field defaults to `true` when a new config is created.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ThumbnailTypes {
    /// Generate thumbnails for image files (PNG, JPG, GIF, WebP, etc.)
    pub images: bool,
    /// Generate thumbnails for video files (MP4, MKV, WebM, etc.)
    pub videos: bool,
    /// Generate thumbnails for font files (TTF, OTF, WOFF, etc.)
    pub fonts: bool,
    /// Generate thumbnails for PDF documents
    pub pdfs: bool,
    /// Generate thumbnails for Windows PE executable files (.exe).
    pub executables: bool,
    /// Generate thumbnails and extract embedded artwork for audio files (.mp3, .flac, .m4a, etc.).
    pub audio: bool,
}

impl Default for ThumbnailTypes {
    fn default() -> Self {
        Self {
            images: true,
            videos: true,
            fonts: true,
            pdfs: true,
            executables: false,
            audio: true,
        }
    }
}
