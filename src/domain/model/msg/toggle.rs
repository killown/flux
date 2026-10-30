/// What a toggle-socket client asks the running window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleVerb {
    Toggle,
    Show,
    Hide,
}

/// Visibility of the main window after a [`ToggleVerb`] was applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowVisibility {
    Visible,
    Hidden,
}
