use rusqlite::Connection;
use std::sync::Mutex;

mod core;
mod history;
mod icons;
mod maintenance;
mod tags;
mod util;
mod views;

pub use util::rekey_path_prefix;

/// Persistent state manager for flux.
pub struct StateManager {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for StateManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateManager").finish()
    }
}
