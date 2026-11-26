//! Saves the open tabs after each change and restores them at startup.

use crate::app::state::FluxApp;
use std::path::PathBuf;

impl FluxApp {
    /// The open tab paths in tab order and the index of the active tab. The active
    /// tab's path is read from `current_path`, which is not copied into `tabs` until
    /// the tab is switched away from.
    pub fn open_tabs_snapshot(&self) -> (Vec<PathBuf>, usize) {
        let paths = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                if i == self.active_tab_index {
                    self.current_path.clone()
                } else {
                    tab.current_path.clone()
                }
            })
            .collect();
        (paths, self.active_tab_index)
    }

    /// Writes the open tabs to the state DB when they differ from the last write.
    pub fn persist_open_tabs(&mut self) {
        if !self.open_tabs_persist || !self.config.ui.restore_tabs {
            return;
        }
        let snapshot = self.open_tabs_snapshot();
        if self.saved_open_tabs.as_ref() == Some(&snapshot) {
            return;
        }
        match self.state_db.save_open_tabs(&snapshot.0, snapshot.1) {
            Ok(()) => self.saved_open_tabs = Some(snapshot),
            Err(e) => eprintln!("[flux] Failed to save open tabs: {e}"),
        }
    }
}
