//! File properties and metadata.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PropertiesMsg {
    /// Initiates a background scan and opens the native directory inspector dialog
    /// for the specified directory path.
    InspectDirectory(std::path::PathBuf),
    /// Opens the right-side Properties panel on this path, or retargets it if already open.
    #[allow(dead_code)]
    OpenFileProperties(PathBuf),
    /// Toggles the right-side Properties panel (its header close button).
    ToggleFilePropertiesPanel,
    /// Background result of a Properties load: the `(title, rows)` sections and,
    /// for a regular file, the name of its default application. Dropped unless
    /// `session` is still current.
    FilePropertiesReady {
        session: u64,
        sections: Vec<(String, Vec<(String, String)>)>,
        open_with: Option<String>,
    },
    /// Copies one Properties row value to the clipboard.
    CopyPropertyValue(String),
    /// Opens the Open With dialog for the file shown in the Properties panel.
    OpenPropertiesTargetWith,
    /// Delivers the result of an async media duration probe for status bar display.
    ///
    /// Carries `Some(duration)` on success or `None` if the file is not a
    /// media container or `ffprobe` is unavailable.
    MediaDurationReady(Option<std::time::Duration>),
    /// Delivers async file metadata (MIME type and optional image dimensions)
    FileMetaReady {
        mime: String,
        dimensions: Option<(u32, u32)>,
    },
}
