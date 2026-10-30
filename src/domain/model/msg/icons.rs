//! Custom icons, MIME icon generation and icon settings.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum IconsMsg {
    /// Toggle whether symbolic link indicators and badges are displayed.
    SetShowSymlinkEmblem(bool),
    /// Sets the base page body hex color for auto-generated extension icons and triggers a cache invalidation.
    SetAutoMimeBodyColor(String),
    /// Sets the label font hex color for auto-generated extension icons and triggers a cache invalidation.
    SetAutoMimeFontColor(String),
    /// Toggles automatic generation of SVG icons for unknown extensions.
    SetAutoGenerateMimeIcons(bool),
    /// Sets the accent color for auto-generated extension icons.
    SetAutoMimeAccentColor(String),
    /// Sets the base font size for auto-generated extension icons.
    SetAutoMimeFontSize(f64),
    /// Removes any custom icon override associated with the given file extension.
    ResetExtensionIcon(String),
    /// Toggles the empty folder emblem feature.
    SetShowEmptyDirEmblem(bool),
    ///Custom icons appear a frame after the directory renders
    FolderIconsReady {
        icons: std::collections::HashMap<String, String>,
        session: u64,
    },
    /// Persists a custom image path as the visual override for a specific file.
    SetFileIcon { path: PathBuf, image_path: PathBuf },
    /// Removes the custom image override for the given file path, restoring the default icon.
    ResetFileIcon(PathBuf),
    /// Internal trigger to open icon picker (resolves path from selection/current dir).
    TriggerIconPicker,
    /// Internal trigger to reset icon (resolves path from selection/current dir).
    TriggerResetIcon,
    /// Updates the default icon size.
    SetIconSize(i32),
    /// Updates the default icon size for list mode view.
    SetListIconSize(i32),
    /// Opens an icon picker dialog for the given directory path.
    ShowIconPicker(PathBuf),
    /// Persists a custom icon name for the given directory path.
    SetFolderIcon { path: PathBuf, icon_name: String },
    /// Removes the custom icon override for the given directory path, restoring the default.
    ResetFolderIcon(PathBuf),
}
