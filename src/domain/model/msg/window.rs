//! Main window, toasts and help/about dialogs.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WindowMsg {
    /// Toggles the visibility state of the top header bar.
    ToggleHeaderBar,
    /// Moves the window control buttons (close, minimize, maximize) to the left
    SetWindowControlsLeft(bool),
    /// Open the memory debug window (Ctrl+Shift+F7).
    OpenDebugWindow,
    /// Updates the startup window width.
    SetWindowWidth(i32),
    /// Updates the startup window height.
    SetWindowHeight(i32),
    /// Applies a visibility verb from the toggle socket and answers the resulting state.
    ToggleWindowVisibility {
        verb: ToggleVerb,
        reply: std::sync::mpsc::Sender<WindowVisibility>,
    },
    /// Open the keyboard shortcuts and help overlay.
    ShowHelp,
    /// Updates the configuration to reflect whether the window is currently maximized.
    SetMaximized(bool),
    /// This variant is used to provide brief, non-blocking feedback to the user
    ShowToast(String),
    /// Opens the About dialog with application metadata and repository link.
    ShowAbout,
}
