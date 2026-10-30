//! Command output dialog.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CommandMsg {
    /// Open the command output log dialog for command task `id` only if still active.
    ShowCommandDialogIfActive(u64),

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
}
