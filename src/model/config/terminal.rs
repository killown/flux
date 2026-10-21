use serde::{Deserialize, Serialize};

/// Where the embedded terminal opens when toggled with F4.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TerminalPosition {
    /// Bottom pane via the vertical `gtk::Paned`. The historical default.
    #[default]
    Bottom,
    /// Right-side sliding panel.
    Sidebar,
    /// Full-window overlay; the whole file-manager UI is hidden while active.
    Fullscreen,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TerminalConfig {
    pub height: i32,
    pub fg_color: String,
    pub bg_color: String,
    pub font: String,
    pub shell: Option<String>,
    /// Where F4 opens the terminal.
    #[serde(default)]
    pub position: TerminalPosition,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            height: 30,
            fg_color: "#E5E5E5".to_string(),
            bg_color: "#1A1A1A".to_string(),
            font: "JetBrains Mono 13".to_string(),
            shell: None,
            position: TerminalPosition::default(),
        }
    }
}
