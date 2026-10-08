use crate::model::{BackgroundSlot, ConflictResolver, FileLoadContext, SortBy};
use crate::ui::conflict_policy::ConflictContext;
use gtk::gdk;
use std::path::PathBuf;

/// Enumeration of all messages handled by the application's update loop.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppMsg {
    /// Opens the currently active diff target at the specified line number with an optional forward search pattern.
    OpenActiveDiffLine { line: usize, query: Option<String> },
    /// Toggles the visibility of the right-side git diff review sidebar.
    ToggleDiffPanel,
    /// Requests the git diff for a specific file and reveals the diff panel.
    ShowFileDiff(PathBuf),
    /// Delivers the loaded git diff output for a target file to update the view.
    DiffLoaded { path: PathBuf, diff: String },
    /// Updates and persists the width of the git diff review panel.
    SetDiffPanelWidth(i32),
    /// Toggles whether selecting a modified file automatically opens the diff sidebar.
    SetAutoShowDiff(bool),
    /// Updates the global UI scale factor across the application.
    SetUiScale(f64),
    /// Filters grid view to files with uncommitted git changes.
    ShowGitStatusView,
    /// Toggles the header git status button.
    SetGitRepoActive(bool),
    /// Emitted when background git status scan finishes.
    GitStatusReady {
        path: PathBuf,
        load_id: u64,
        updates: std::collections::HashMap<PathBuf, crate::services::git::GitFileStatus>,
    },
    /// Toggles the background SQLite FTS5 file indexer.
    SetEnableFileIndexing(bool),
    /// Opens multiple directory paths into tabs in a single batch.
    OpenTabs(Vec<PathBuf>),
    /// Opens a new window tab with an optional initial directory path.
    NewTab(Option<PathBuf>),
    /// Closes a tab by index, or closes the currently active tab if `None`.
    CloseTab(Option<usize>),
    /// Switches keyboard focus and display to the tab at the given index.
    SwitchTab(usize),
    /// Cycles to the next tab in sequential order.
    NextTab,
    /// Cycles to the previous tab in reverse sequential order.
    PrevTab,
    /// Notifies that the vertical viewport changed with normalized scroll progress values.
    UpdateVisibleThumbnailsViewport {
        progress_top: f64,
        progress_bottom: f64,
    },
    /// Updates the tint overlay opacity for a background slot and re-renders the CSS.
    SetBackgroundAlpha { slot: BackgroundSlot, alpha: f64 },
    /// Deletes all custom background image files from the local data directory
    /// and resets all background styling across the window and sidebars.
    ClearFluxBackgrounds,
    /// Copies an image file to the designated application background slot
    /// (`~/.local/share/flux/data/resources/images/`) and dynamically injects
    /// the updated CSS provider styles to refresh the view immediately.
    SetFluxBackground {
        target: PathBuf,
        slot: BackgroundSlot,
    },
    /// Toggles dynamic scaling of file label fonts with grid icon size.
    SetScaleFontWithIcons(bool),
    /// Updates the list of file extensions hidden from grid labels and refreshes the view.
    SetHiddenExtensions(Vec<String>),
    /// Persists and updates the tag panel width in pixels.
    SetTagPanelWidth(i32),
    /// Persists and updates the search panel width in pixels.
    SetSearchPanelWidth(i32),
    /// Persists the provided list of tags to both the database index and extended attributes
    /// (`user.xdg.tags`) for all items currently selected in the file view.
    ApplyTagsToSelection(Vec<String>),
    /// Removes a specific tag from all currently selected files and persists the updated metadata.
    RemoveTagFromSelection(String),
    /// Toggles the visibility of the lazy-initialized right tag navigator sidebar panel.
    ToggleTagPanel,
    /// Toggles the visibility of the lazy-initialized right search sidebar panel.
    ToggleSearchPanel,
    /// Activates the spinner after the debounce timeout if the session is still current.
    ShowLoadingSpinner(u64),
    /// Toggle whether symbolic link indicators and badges are displayed.
    SetShowSymlinkEmblem(bool),
    /// Toggles the visibility state of the bottom status bar.
    ToggleStatusBar,
    /// Pin a specific tag to the user's sidebar bookmarks in `config.toml`.
    AddTagToSidebar(String),
    /// Message triggered when the file grid scroll position reaches or leaves the bottom boundary.
    SetScrolledToBottom(bool),
    /// Initiates a copy or move of selected items directly to a quick list destination.
    PerformQuickTransfer { dest: PathBuf, is_cut: bool },
    /// Overwrites the quick-list entry at the given index with the current directory path.
    UpdateExclusiveSlot(usize),
    /// Initiates a background scan and opens the native directory inspector dialog
    /// for the specified directory path.
    InspectDirectory(std::path::PathBuf),
    /// Opens the custom Open With dialog for a given path.
    ShowOpenWithDialog(PathBuf),
    /// Terminal changed directory via OSC 7.
    TerminalCwdChanged(PathBuf),
    /// Toggles the visibility state of the top header bar.
    ToggleHeaderBar,
    /// Toggles whether folders are grouped first or last for the current directory.
    ToggleCurrentFoldersFirst,
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
    /// Copy absolute paths of selected items to clipboard.
    CopyPath,
    /// Create symbolic links from clipboard paths.
    CreateSymlink,
    /// Create hard links from clipboard paths (directories not supported).
    CreateHardlink,
    /// Prompts the user for confirmation before permanently deleting an entry from an archive.
    PromptArchiveDeletion {
        archive_path: PathBuf,
        inner_path: String,
    },
    /// Toggles automatic muted video playback on card selection.
    SetAutoplayVideoPreviews(bool),
    /// Triggers deferred inline video playback after a debounce timeout.
    TriggerVideoPreview(PathBuf),
    /// Updates the target pixel dimension for rendered and cached thumbnails.
    SetThumbnailSize(i32),
    /// Sets a custom shell executable path for the embedded terminal (e.g., `Some("/bin/fish".to_string())`).
    SetTerminalShell(Option<String>),
    /// Extracts the currently browsed archive to a sibling folder named.
    ExtractArchive,
    /// Moves the window control buttons (close, minimize, maximize) to the left
    SetWindowControlsLeft(bool),
    /// Toggles the empty folder emblem feature.
    SetShowEmptyDirEmblem(bool),
    /// Evicts the current directory from the folder cache and navigates to the given path.
    InvalidateCacheAndNavigate(PathBuf),
    /// Set the thread count used by each FFmpeg thumbnail process.
    SetFfmpegThreads(usize),
    /// Set the seek offset (in seconds) for video thumbnail frame grabs.
    SetFfmpegSeekSeconds(f64),
    /// Toggle automatic rotation based on video container metadata.
    SetFfmpegAutoRotate(bool),
    /// Set the batch size for streaming directory entries into the UI grid.
    SetLoaderBatchSize(usize),
    /// Set the capacity of the in-memory folder cache.
    SetFolderCacheCapacity(usize),
    /// Set the maximum concurrent thumbnail rendering tasks.
    SetThumbnailThreads(usize),
    /// Set the maximum search results limit.
    SetMaxSearchResults(usize),
    /// Set the maximum navigation history depth.
    SetMaxHistory(usize),
    /// Open the memory debug window (Ctrl+Shift+F7).
    OpenDebugWindow,
    /// Delivers an incremental batch of load contexts to the grid view.
    FolderLoadedChunk {
        load_id: u64,
        chunk: Vec<FileLoadContext>,
        is_cached: bool,
    },
    /// Finalizes directory loading after all chunks are appended.
    FolderLoadedFinish { load_id: u64 },
    /// Toggles the drag-and-drop feature across the grid and sidebar.
    SetDisableDragAndDrop(bool),
    /// Toggles the lazy thumbnail generation setting.
    SetLazyThumbnails(bool),
    /// Check for visible thumbnails in the current viewport and request generation for any missing ones
    CheckVisibleThumbnails,
    /// Delivers background-enumerated system and network mounts to update the sidebar.
    SystemMountsReady(Vec<(String, std::path::PathBuf)>),
    ///Custom icons appear a frame after the directory renders
    FolderIconsReady {
        icons: std::collections::HashMap<String, String>,
        session: u64,
    },
    /// Sets the maximum number of content search results.
    /// Value is persisted in the config file.
    SetMaxContentSearchResults(usize),
    /// A batch of files matched a recursive extension/glob search.
    ExtensionSearchBatch {
        results: Vec<crate::services::search::ExtensionMatch>,
        session: u64,
    },
    /// Kick off a recursive filename search using glob patterns.
    StartExtensionSearch(Vec<String>),
    /// Recursive search with full advanced constraints.
    StartAdvancedSearch(crate::services::search::AdvancedSearchParams),
    /// Paste image data from the clipboard as a timestamped file into the current directory.
    PasteImageFromClipboard,
    /// Paste plain text as a file.
    PasteTextFromClipboard,
    /// Paste rich text HTML content from the clipboard into a new `.html` file.
    PasteHtmlFromClipboard,
    /// Delete a tag from the database entirely across all files.
    DeleteTagGlobally(String),
    /// Open the tag picker popover for the current selection (Ctrl+T).
    OpenTagPicker,
    /// Deliver the resolved tags for the picker to display.
    TagsReady {
        paths: Vec<PathBuf>,
        tags: Vec<String>,
        available_tags: Vec<String>,
    },
    /// Persist a tag set to xattr + SQLite for the given path.
    SetFileTags { path: PathBuf, tags: Vec<String> },
    /// Navigate to a virtual tag view or filter by tag.
    NavigateTag(String),
    /// Prompts a modal dialog to rename a section label.
    PromptSidebarRenameSection {
        old_name: String,
        current_name: String,
    },
    /// Persists a new name for a section label entry in config.toml.
    RenameSidebarSection { old_name: String, new_name: String },
    /// Removes a section label entry from config.toml and refreshes the sidebar.
    RemoveSidebarSection(String),
    /// Prompts the user for a title, then appends a new `kind = "label"`.
    PromptNewSidebarSection,
    /// Appends a new section-label entry with the given title to config.sidebar.
    AddSidebarSection(String),
    /// Show or hide the "Pin to Sidebar" drop zone at the bottom of the sidebar.
    ShowSidebarPinZone(bool),
    /// Files were dragged from the grid and dropped onto a quick-list tab button.
    MoveFilesToTarget {
        sources: Vec<PathBuf>,
        destination: PathBuf,
    },
    /// Prompts a dialog to rename a sidebar bookmark.
    PromptSidebarRename { path: PathBuf, current_name: String },
    /// Persists a new name for a matching sidebar entry in config.toml.
    RenameSidebarPlace { path: PathBuf, new_name: String },
    /// Undo the most recent file operation (Ctrl+Z).
    Undo,
    /// Redo the most recently undone operation (Ctrl+Shift+Z / Ctrl+Y).
    Redo,
    /// Internal: background undo-move succeeded, record the inverse redo op.
    UndoMoveComplete {
        redo_items: Vec<(std::path::PathBuf, std::path::PathBuf)>,
        dest_dir: std::path::PathBuf,
    },
    /// Internal: background undo-move failed, restore op on the undo stack.
    UndoMoveFailed(crate::ui::undo_redo::FileOp),
    /// Internal: background undo-trash succeeded, record the inverse redo op.
    UndoTrashComplete { paths: Vec<std::path::PathBuf> },
    /// Internal: background undo-trash failed, restore op on the undo stack.
    UndoTrashFailed(crate::ui::undo_redo::FileOp),
    /// Internal: background redo-move succeeded, record the inverse undo op.
    RedoMoveComplete {
        items: Vec<(std::path::PathBuf, std::path::PathBuf)>,
        dest_dir: std::path::PathBuf,
    },
    /// Internal: background redo-move failed, restore op on the redo stack.
    RedoMoveFailed(crate::ui::undo_redo::FileOp),
    /// Internal: background redo-trash succeeded, record the inverse undo op.
    RedoTrashComplete { paths: Vec<std::path::PathBuf> },
    /// Internal: background redo-trash failed, restore op on the redo stack.
    RedoTrashFailed(crate::ui::undo_redo::FileOp),
    /// Notification that items were sent to the trash.
    TrashSucceeded(Vec<std::path::PathBuf>),
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
    /// A background copy/move worker detected that its destination path already
    /// exists and needs user input before it can proceed.
    FileConflictDetected {
        context: ConflictContext,
        resolver: ConflictResolver,
    },
    /// Sent by the conflict dialog's response closure immediately before it
    /// resolves the oneshot sender.  Clears `FluxApp::conflict_dialog_active`
    /// so the transfer-progress dialog is re-enabled.
    ConflictDialogClosed,
    /// Update (or reset) the session-scoped conflict-resolution policy for the
    /// currently executing batch.  Sent by `conflict_dialog` when the user
    /// enables "Apply to all".
    SetConflictPolicy(crate::ui::conflict_policy::ConflictPolicy),
    /// Open the command output log dialog for command task `id` only if still active.
    ShowCommandDialogIfActive(u64),
    /// Ctrl+Right-click on a file: kicks off async MIME resolution for the
    /// secondary template-driven context menu.
    PrepareSecondaryMenu {
        x: f64,
        y: f64,
        path: Option<PathBuf>,
    },
    /// Delivers the fully-resolved secondary menu actions to the GTK main
    /// thread for popover construction.
    ShowSecondaryMenu {
        x: f64,
        y: f64,
        path: Option<PathBuf>,
        mime: String,
        actions: Vec<crate::model::CustomAction>,
    },

    /// Toggle the `no_command_dialog` flag for a specific menu action by its action name.
    /// This persists the change to the config file.
    ///
    /// # Arguments
    /// * `String` – The unique action name (e.g., `custom_0`) of the menu entry.
    ToggleNoCommandDialog(String),

    /// Refresh the command dialog's switch state after a toggle.
    /// Sent from the update loop to an open command dialog.
    ///
    /// # Arguments
    /// * `String` – The action name whose switch state should be updated.
    RefreshCommandDialog(String),
    /// The command output dialog was closed (by the user or automatically).
    ///
    /// This clears the `FluxApp::command_dialog` handle so that subsequent
    /// commands can open a new dialog.
    CommandDialogClosed,
    /// Open the command output log dialog for command task `id`.
    ShowCommandDialog(u64),
    /// Syncs the inline path entry widget text to the current directory.
    SyncPathEntry,
    /// Append a line of output from a command task.
    CommandOutput {
        id: u64,
        line: String,
        is_stderr: bool,
    },

    /// Command finished (success or failure).
    CommandFinished {
        id: u64,
        success: bool,
        exit_code: Option<i32>,
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
    /// Persists a custom image path as the visual override for a specific file.
    SetFileIcon { path: PathBuf, image_path: PathBuf },
    /// Removes the custom image override for the given file path, restoring the default icon.
    ResetFileIcon(PathBuf),
    /// Dispatched when a LUKS image file is double-clicked and confirmed as LUKS.
    UnlockLuksImage { path: PathBuf },
    /// Dispatched by the background thread after successful unlock + mount.
    LuksMounted {
        image_path: PathBuf,
        mount_point: PathBuf,
    },
    /// Delivers the result of an async archive directory listing.
    ArchiveLoaded {
        archive_path: PathBuf,
        prefix: String,
        password: Option<String>,
        /// Session identifier captured at spawn time, used to discard results
        /// from background tasks that were superseded by a subsequent navigation.
        load_id: u64,
        result: Result<
            Vec<crate::services::archive::ArchiveEntry>,
            crate::services::archive::ArchiveError,
        >,
    },
    /// Initiates a deep content search within the current directory.
    /// Syntax: `:term` to search all files, or `:.ext:term` to filter by extension
    /// (e.g., `:.rs:hello` for `.rs` files). Multiple extensions can be comma‑separated
    /// (e.g., `:.rs,py:hello`). `term` must be at least 3 characters.
    /// The second argument is the optional extension filter (without the dot).
    StartContentSearch(String, Option<String>),
    /// Cancels any in-flight content search.
    CancelContentSearch,
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
    /// Toggles the right location panel (Ctrl+L).
    ToggleLocationPanel,
    /// Delivers the result of an async network directory listing.
    ///
    /// Analogous to `ArchiveLoaded`, carries the pre-computed load contexts so
    /// the GTK main thread can populate the grid without touching GIO directly.
    NetworkLoaded {
        uri: String,
        contexts: Vec<crate::model::FileLoadContext>,
    },
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
    },
    /// Navigate to the `network:///` GIO location that lists visible network neighbours.
    NavigateNetwork,

    /// Connect to a remote server using the supplied connection parameters.
    ///
    /// Dispatched by the "Connect to Server" dialog. The URI is built from
    /// [`crate::services::network::ConnectToServerParams::build_uri`] and then
    /// handled identically to `Navigate(PathBuf::from(uri))`.
    ConnectToServer {
        /// The canonical GIO URI of the remote location (e.g. `smb://server/share`).
        uri: String,
        /// Optional credentials to pre-populate the GVFS mount operation.
        credentials: Option<crate::services::network::NetworkCredentials>,
    },

    /// The remote server at `uri` requires credentials before the listing can proceed.
    ///
    /// Dispatched by `load_network` when [`crate::services::network::NetworkError::CredentialsRequired`]
    /// is returned. The update loop shows the GTK credentials dialog and, on
    /// confirmation, re-dispatches [`AppMsg::ConnectToServer`] with the filled credentials.
    PromptNetworkCredentials {
        uri: String,
        /// Human-readable prompt from the GVFS backend.
        message: String,
        /// Which fields the server needs.
        flags: crate::services::network::NetworkAuthFlags,
        /// `true` when a previous attempt was rejected - the dialog shows an error hint.
        auth_failed: bool,
    },

    /// Persist a network location as a sidebar bookmark.
    AddNetworkBookmark { name: String, uri: String },

    /// Remove a network bookmark by URI.
    RemoveNetworkBookmark(String),

    /// Unmount the GVFS mount backing the given network URI.
    UnmountNetwork(String),

    /// Rebuild the "Network" section in the sidebar from the current active GVFS mounts.
    RefreshNetworkSidebar,

    /// Sets the zero-based insertion index for the Recents row within the `[[sidebar]]` entry list.
    ///
    /// Values ≥ the number of configured sidebar entries place Recents after all of them.
    /// Has no effect when `show_recents` is `false`.
    SetRecentsRow(usize),
    /// Remove one or all entries from the recent-files list.
    ClearRecents,
    /// Toggle the embedded terminal panel visibility.
    ToggleTerminal,
    /// Internal trigger to open icon picker (resolves path from selection/current dir).
    TriggerIconPicker,
    /// Internal trigger to reset icon (resolves path from selection/current dir).
    TriggerResetIcon,
    /// Updates the startup window width.
    SetWindowWidth(i32),
    /// Updates the startup window height.
    SetWindowHeight(i32),
    /// Signals that the file selection set in the main grid has changed.
    SelectionChanged,
    /// Updates the single-click activation setting.
    SetSingleClick(bool),
    /// Updates the global hidden files visibility.
    SetShowHidden(bool),
    /// Updates the folders-first sorting priority.
    SetFoldersFirst(bool),
    /// Updates the default icon size.
    SetIconSize(i32),
    /// Updates the default icon size for list mode view.
    SetListIconSize(i32),
    /// Updates the preferred sidebar width.
    SetSidebarWidth(i32),
    /// Toggles Client-Side Decorations (CSD).
    SetShowCsd(bool),
    /// Toggles visibility of standard XDG directories in the sidebar.
    SetShowXdgDirs(bool),
    /// Updates the active UI theme name.
    SetTheme(Option<String>),
    /// Updates the default sorting method.
    SetDefaultSort(SortBy),
    /// Updates a specific keyboard shortcut.
    SetShortcut(String, Option<String>),
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
    /// Launches the currently selected file(s) using a specific application.
    ///
    /// This variant bypasses thread-safety restrictions of `gio::AppInfo` by passing
    /// the application's Desktop ID (e.g., "org.gnome.gedit.desktop") as a `String`.
    /// The actual `GAppInfo` is resolved on the main thread within the update loop
    /// using `gio::DesktopAppInfo::new()`.
    LaunchWithApp(String),
    /// Calculate coordinates and determine target for a context menu.
    PrepareContextMenu(f64, f64, Option<PathBuf>),
    /// Display the context menu popover with relevant actions for the given mime type.
    ShowContextMenu {
        x: f64,
        y: f64,
        path: Option<PathBuf>,
        mime: String,
    },
    #[allow(dead_code)]
    OpenFileProperties(PathBuf),
    /// Update the current path and refresh the file list.
    Navigate(PathBuf),
    /// Finalize a file or directory rename operation.
    PerformRename(PathBuf, String),
    /// Navigate to a specific index in the recent folders stack.
    JumpToRecent(usize),
    /// Append a character to the active search filter.
    SearchInput(char),
    /// Remove the last character from the active search filter.
    SearchBackspace,
    /// Synchronize the search entry state with the model.
    CloseSearchSync,
    /// Add a directory to the exclusive navigation list.
    ///
    /// `Some(path)` targets an explicit path (e.g. from a context menu action).
    /// `None` resolves the path from the current selection or working directory,
    /// which is the behaviour of the `Insert` keyboard shortcut.
    AddExclusive(Option<PathBuf>),
    /// Clear all items from the exclusive navigation list.
    ClearExclusive,
    /// Switch to the next directory in the exclusive list.
    NextExclusive,
    /// Switch to the previous directory in the exclusive list.
    PrevExclusive,
    /// Enter inline rename mode for the specified file.
    StartRename(PathBuf),
    /// Execute the primary action for all currently selected items.
    Activate,
    /// Trigger the rename state for the currently selected item.
    TriggerRenameSelection,
    #[allow(dead_code)]
    ToggleSingleClick,
    /// Force a rebuild of the sidebar entries.
    RefreshSidebar,
    /// Remove a custom entry from the sidebar by its resolved path.
    RemoveFromSidebar(PathBuf),
    /// Permanently add the selected folder (or current directory) to config.toml sidebar entries.
    AddToSidebarPermanent,
    /// Reorder a custom sidebar entry: move `from` path to the position currently held by `to`.
    ReorderSidebar { from: PathBuf, to: PathBuf },
    /// Pin `path` into the sidebar immediately before the row whose config path matches `before`.
    ///
    /// Emitted when a folder is dragged from the grid and dropped onto a specific sidebar row.
    /// If `before` is not found (e.g. a section label or unmapped mount), appends to the end
    /// of the user-defined sidebar entries.
    PinFolderAt {
        path: PathBuf,
        before: PathBuf,
        label_name: Option<String>,
    },
    /// Move files dragged from the grid and dropped onto a non-folder sidebar row's target folder.
    SidebarDropMove {
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
    },
    /// Open the keyboard shortcuts and help overlay.
    ShowHelp,
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
    /// Toggle visibility of dotfiles and hidden items.
    ToggleHidden,
    /// Switch between grid card layout and compact list layout.
    ToggleListMode,
    /// Rotate through available sorting methods.
    CycleSort,
    /// Toggle between "Folders First" and mixed sorting.
    CycleFolderPriority,
    /// Update the string used to filter the current view.
    UpdateFilter(String),
    /// Persist a new set of glob patterns as the session-scoped navigation filter.
    ///
    /// Patterns use shell glob syntax (`*`, `?`) and are matched case-insensitively
    /// against each file's full name (not just its extension), allowing patterns
    /// like `*.py`, `a*`, or `report??.pdf`. An empty `Vec` is treated the same
    /// as [`AppMsg::ClearExtensionFilter`].
    SetExtensionFilter(Vec<String>),
    /// Removes all active session-scoped navigation filter patterns.
    ///
    /// Equivalent to `SetExtensionFilter(vec![])`. Directories are always visible
    /// regardless of filter state, clearing restores files to their unfiltered listing.
    ClearExtensionFilter,
    /// Signal that a thumbnail has been successfully generated.
    /// A file matched a content search query. Carries the path, the matching
    /// line text, and the session id for stale-result rejection.
    ContentSearchResult {
        path: PathBuf,
        line: String,
        line_number: usize,
        session: u64,
    },
    /// The content search walk finished (or was cancelled). Used to clear the
    /// loading indicator and select the first result.
    ContentSearchDone { session: u64 },
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
    /// Switch the header bar between path, entry, and search modes.
    SwitchHeader(String),
    /// Execute a shell command.
    ExecuteCommand(String),
    /// Adjust the icon size based on scroll delta.
    Zoom(f64),
    /// Move back in history.
    GoBack,
    /// Move forward in history.
    GoForward,
    /// Reload the current directory from disk.
    Refresh,
    /// Enter a compressed archive and browse its contents as a virtual directory.
    ///
    /// The `PathBuf` is the real on-disk path of the archive file. The app
    /// synthesises an `archive://` URI internally and delegates rendering to
    /// `load_archive` in `services/loader.rs`.
    EnterArchive(PathBuf),
    /// Show a password dialog then retry loading the archive.
    ///
    /// Triggered by [`ArchiveError::PasswordRequired`] or [`ArchiveError::WrongPassword`]
    /// from `load_archive`. The dialog dispatches [`AppMsg::LoadArchiveWithPassword`] on
    /// confirmation.
    PromptArchivePassword {
        archive_path: PathBuf,
        prefix: String,
        /// `true` when a previous attempt failed - shows an error hint in the dialog.
        wrong_password: bool,
    },
    /// Retry loading an archive after the user supplied a password.
    LoadArchiveWithPassword {
        archive_path: PathBuf,
        prefix: String,
        password: String,
    },
    /// Open all currently selected files or navigate to the selected directory.
    ///
    /// `Some(position)` is supplied by `GridView::connect_activate` and addresses
    /// the item directly without querying the selection model. `None` is used by
    /// keyboard shortcuts and falls back to `get_selection_with_meta()`.
    Open(Option<u32>),
    /// Delete all files within the system trash location.
    EmptyTrash,
    #[allow(dead_code)]
    RestoreItem(PathBuf),
    /// Updates the configuration to reflect whether the window is currently maximized.
    SetMaximized(bool),
    /// The path of a file that has been removed from the filesystem.
    FileDeleted(std::path::PathBuf),
    /// The path of a file whose contents or metadata have been modified.
    FileChanged(std::path::PathBuf),
    /// Report progress for a specific background operation slot.
    TaskProgress {
        id: u64,
        /// Human-readable description (filename or "N files").
        label: String,
        current: u64,
        total: u64,
        total_items: usize,
        cancellable: gio::Cancellable,
    },
    /// Signal that a specific background operation has completed.
    TaskCompleted(u64),
    #[allow(dead_code)]
    /// Cancel a single in-flight background operation by its task ID.
    CancelTask(u64),
    /// Cancel every in-flight background operation immediately.
    CancelAllTasks,
    /// Throttled tick to refresh the status bar from the task queue.
    TaskQueueTick,
    /// Open the transfer progress dialog (idempotent - no-op if already open
    /// or if the queue drained before the message was processed).
    ShowTransferDialog,
    /// Show the dialog only when task `id` is still active.
    ///
    /// Dispatched by the 2-second delay timer in `paste_ops.rs`, prevents
    /// opening an empty dialog when a fast copy already completed.
    ShowTransferDialogIfActive(u64),
    /// The transfer dialog window was closed (by the user or because the
    /// queue drained).  Clears `FluxApp::transfer_dialog`.
    TransferDialogClosed,
    /// This variant is used to provide brief, non-blocking feedback to the user
    ShowToast(String),
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
    /// Triggers the asynchronous unmounting of a system drive or mounted volume.
    UnmountDevice(std::path::PathBuf),
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
    /// Updates the pixel spacing between items in the grid.
    SetGridSpacing(i32),
    /// Updates the character limit for file labels.
    SetMaxWidthChars(i32),
    /// Toggles multi-line label wrapping in the grid.
    SetExpandLabels(bool),
    /// Toggles the directory listing between ascending and descending order.
    ToggleSortOrder,
    /// Sets sort direction: true for Ascending, false for Descending.
    SetAsc(bool),
    /// Opens an icon picker dialog for a sidebar entry (symbolic only).
    ShowSidebarIconPicker(PathBuf),
    /// Opens an icon picker dialog for the given directory path.
    ShowIconPicker(PathBuf),
    /// Persists a custom icon name for the given directory path.
    SetFolderIcon { path: PathBuf, icon_name: String },
    /// Removes the custom icon override for the given directory path, restoring the default.
    ResetFolderIcon(PathBuf),
    /// Updates the height of the embedded terminal panel in pixels.
    SetTerminalHeight(i32),
    /// Updates the Pango font description string for the embedded terminal.
    SetTerminalFont(String),
    /// Updates the foreground text color (as a hex string) for the embedded terminal.
    SetTerminalFgColor(String),
    /// Updates the background color (as a hex string) for the embedded terminal.
    SetTerminalBgColor(String),
    /// Opens the About dialog with application metadata and repository link.
    ShowAbout,
    /// Toggles the visibility of the sidebar.
    ToggleSidebar,
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
    /// Toggles the "Recents" virtual entry in the sidebar.
    SetShowRecents(bool),
    /// Remove a single path from the quick-list panel by exact match.
    RemoveQuickItem(PathBuf),
    /// Tear down and reconstruct all tab buttons in the quick-list panel.
    ///
    /// Sent automatically after any mutation of `exclusive_list` so the widget
    /// tree always reflects the current state.
    RebuildQuickPanel,
}
