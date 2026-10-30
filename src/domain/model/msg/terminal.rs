//! Embedded terminal.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TerminalMsg {
    /// Toggles the full-window terminal mode. F4 in Fullscreen position.
    ToggleTerminalFullscreen,
    /// Sets the terminal position (Bottom / Sidebar / Fullscreen) and persists it.
    /// Emitted by the Settings dropdown.
    SetTerminalPosition(crate::domain::model::TerminalPosition),
    /// Toggles the visibility of the right-side terminal panel.
    ToggleTerminalPanel,
    /// Terminal changed directory via OSC 7.
    TerminalCwdChanged(PathBuf),
    /// Sets a custom shell executable path for the embedded terminal (e.g., `Some("/bin/fish".to_string())`).
    SetTerminalShell(Option<String>),
    /// Toggle the embedded terminal panel visibility.
    ToggleTerminal,
    /// Updates the height of the embedded terminal panel in pixels.
    SetTerminalHeight(i32),
    /// Updates the Pango font description string for the embedded terminal.
    SetTerminalFont(String),
    /// Updates the foreground text color (as a hex string) for the embedded terminal.
    SetTerminalFgColor(String),
    /// Updates the background color (as a hex string) for the embedded terminal.
    SetTerminalBgColor(String),
}
