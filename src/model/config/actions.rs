/// Defines a user-configured external command to be displayed in context menus.
#[derive(Clone, Debug)]
pub struct CustomAction {
    /// The text label to be displayed in the context menu.
    pub label: String,
    /// Optional name of a parent menu to group this action under.
    pub submenu: Option<String>,
    /// Unique identifier used to register and trigger the action.
    pub action_name: String,
    /// The shell command string to be executed, supporting path placeholders.
    pub command: String,
    /// A list of supported MIME types or patterns for context-sensitivity.
    pub mime_types: Vec<String>,
    /// Optional toast message to display after the command is dispatched.
    pub toast: Option<String>,
    /// If true, suppresses the transfer dialog for this command.
    pub no_command_dialog: bool,
}

/// Metadata for actions available within a specific UI context.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ContextAction {
    /// The user-facing text displayed in the menu for this action.
    pub label: String,
    /// Unique identifier for the action used by the system to dispatch events.
    pub action_name: String,
    /// The shell command or internal function identifier to execute upon activation.
    pub command: String,
    /// List of file types or categories where this action is valid.
    pub mime_types: Vec<String>,
    /// Generate thumbnails for Windows PE executable files (.exe).
    pub executables: bool,
}

/// Represents a single entry in the Flux context menu configuration.
///
/// This structure maps to the domain-specific language used in `menu.rs`,
/// allowing for the definition of custom actions based on file types.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MenuEntry {
    /// The user-facing text displayed in the menu.
    pub label: String,
    /// An optional category name used to nest this entry within a submenu.
    pub submenu: Option<String>,
    /// Comma-separated patterns (e.g., "image/all", "directory") that define when this entry appears.
    pub mime_types: String,
    /// The shell command to execute, supporting placeholders like %p for the file path.
    pub command: String,
    /// An optional message to display as a toast notification after the command runs.
    pub toast: Option<String>,
    /// If true, suppresses tracking progress and opening the transfer dialog.
    #[serde(default)]
    pub no_command_dialog: bool,
}

impl MenuEntry {
    /// Converts the entry into the DSL string format used in `menu.rs`.
    ///
    /// The output follows the pattern:
    /// `"Submenu > Label" => "mime_types", "command", "toast", "no_command_dialog"`
    pub fn to_config_line(&self) -> String {
        let label_field = match &self.submenu {
            Some(sub) => format!("{} > {}", sub, self.label),
            None => self.label.clone(),
        };

        let mut line = format!(
            r#""{}" => "{}", "{}""#,
            label_field, self.mime_types, self.command
        );

        if let Some(t) = &self.toast {
            line.push_str(&format!(r#", "{}""#, t));
        }

        if self.no_command_dialog {
            line.push_str(r#", "no_command_dialog""#);
        }

        line
    }
}
