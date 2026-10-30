//! Actions panel, contextual actions, open-with and context menus.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ActionsMsg {
    /// Reload the contextual-actions panel after its template file was
    /// edited from the in-panel editor.
    ReloadActionsTemplate { path: PathBuf },
    /// Persists and updates the contextual-actions panel width in pixels.
    SetActionsPanelWidth(i32),
    /// Toggles the right contextual-actions panel (Ctrl+Shift+A).
    ToggleActionsPanel,
    /// Asks the contextual-actions panel to refresh for the current selection.
    /// MIME resolution runs on a blocking worker, `session` guards stale results.
    ActionsPanelLoad { session: u64 },
    /// Delivers the parsed secondary-menu actions for the resolved MIME type.
    /// Stale sessions are dropped before any widget is touched.
    ActionsPanelReady {
        session: u64,
        mime: String,
        actions: Vec<crate::domain::model::CustomAction>,
        template_path: Option<PathBuf>,
    },
    /// Opens the custom Open With dialog for a given path.
    ShowOpenWithDialog(PathBuf),
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
        actions: Vec<crate::domain::model::CustomAction>,
    },
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
    /// Execute a shell command.
    ExecuteCommand(String),
    /// Executes a command from a secondary (`menu-*.rs`) template.
    /// Carries `no_command_dialog`, which is not present in `menu_actions`.
    ExecuteSecondaryAction {
        command: String,
        no_command_dialog: bool,
    },
    /// Opens the "New Action" dialog of the contextual-actions panel for `mime`.
    PromptNewContextualAction { mime: String },
    /// Appends one action to the `menu-*.rs` template for `mime` (off the main thread).
    ///
    /// `category_scope` picks `menu-<category>-all.rs` over `menu-<category>-<subtype>.rs`.
    SaveContextualAction {
        mime: String,
        category_scope: bool,
        label: String,
        submenu: Option<String>,
        mime_types: String,
        command: String,
        toast: Option<String>,
        no_command_dialog: bool,
    },
    /// Result of `SaveContextualAction`: `Err` carries a user-facing message.
    ContextualActionSaved(Result<std::path::PathBuf, String>),
}
