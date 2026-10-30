//! Clipboard, create/rename/delete, drag and drop and file-change events.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum FileOpsMsg {
    /// Copy absolute paths of selected items to clipboard.
    CopyPath,
    /// Result of `handle_copy_path`: the canonicalized, shell-quoted
    /// space-joined selection ready to be placed on the clipboard.
    CopyPathReady(String),
    /// Create symbolic links from clipboard paths.
    CreateSymlink,
    /// Create hard links from clipboard paths (directories not supported).
    CreateHardlink,
    /// Toggles the drag-and-drop feature across the grid and sidebar.
    SetDisableDragAndDrop(bool),
    /// Paste image data from the clipboard as a timestamped file into the current directory.
    PasteImageFromClipboard,
    /// Paste plain text as a file.
    PasteTextFromClipboard,
    /// Paste rich text HTML content from the clipboard into a new `.html` file.
    PasteHtmlFromClipboard,
    /// Notification that items were sent to the trash.
    TrashSucceeded(Vec<crate::ui::undo_redo::TrashedItem>),
    /// Notification that a batch move completed.
    MoveSucceeded {
        items: Vec<(std::path::PathBuf, std::path::PathBuf)>,
        dest_dir: std::path::PathBuf,
    },
    /// Notification that a batch copy completed.
    CopySucceeded {
        copies: Vec<(std::path::PathBuf, std::path::PathBuf)>,
        dest_dir: std::path::PathBuf,
    },
    /// Re-keys custom icon entries in both `file_icons` and `folder_icons` after
    /// a successful filesystem move, then triggers a view refresh.
    ///
    /// Dispatched from the background move threads in `handle_drop_items`,
    /// `dispatch_paste_ops`, and `perform_paste_inner` whenever `is_cut` is true
    /// and the GIO move succeeded. `old_path` is the pre-move location,
    /// `new_path` is the post-move destination.
    ItemMoved {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    /// Installs the given font files into the user font directory.
    InstallFonts(Vec<PathBuf>),
    /// Presents a modal GTK dialog allowing the user to type a name for a new
    /// folder to be created inside the current directory.
    ///
    /// Works for both local filesystem paths and network URIs (SMB, SFTP, FTP).
    PromptNewFolder,
    /// Presents a modal GTK dialog allowing the user to type a name for a new
    /// empty file to be created inside the current directory.
    ///
    /// Works for both local filesystem paths and network URIs (SMB, SFTP, FTP).
    PromptNewFile,
    /// Copy the current selection to the clipboard with a "copy" intent.
    Copy,
    /// Copy the current selection to the clipboard with a "cut" intent.
    Cut,
    /// Request data from the clipboard and trigger a Move or Copy.
    Paste,
    /// Triggers the process to move selected files to the system trash.
    Delete,
    /// Internal message to execute the file operations after clipboard data is retrieved.
    PerformPaste { files: Vec<gio::File>, is_cut: bool },
    /// Finalize a file or directory rename operation.
    PerformRename(PathBuf, String),
    /// Enter inline rename mode for the specified file.
    StartRename(PathBuf),
    /// Trigger the rename state for the currently selected item.
    TriggerRenameSelection,
    /// Handle a Drag-and-Drop move/copy operation for multiple items.
    HandleDrop {
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
    },
    /// Handles cross-instance file moves via `text/uri-list` serialization.
    HandleExternalDrop {
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
    },
    /// Delete all files within the system trash location.
    EmptyTrash,
    #[allow(dead_code)]
    RestoreItem(PathBuf),
    /// The path of a file that has been removed from the filesystem.
    FileDeleted(std::path::PathBuf),
    /// The path of a file whose contents or metadata have been modified.
    FileChanged(std::path::PathBuf),
    /// Pause a paste operation and ask the user whether to replace conflicting directories.
    ///
    /// Carries the full original file list so it can be resumed via `PerformPasteForced`
    /// and the display names of the conflicting items for the dialog body.
    ConfirmReplacePaste {
        files: Vec<gio::File>,
        conflicts: Vec<String>,
        is_cut: bool,
    },
    /// Resume a paste operation after the user has confirmed directory replacement.
    ///
    /// Identical to `PerformPaste` but skips conflict detection and always passes
    /// `FileCopyFlags::OVERWRITE` for both copy and move operations.
    PerformPasteForced { files: Vec<gio::File>, is_cut: bool },
}
