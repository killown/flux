use super::StateManager;
use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SCHEMA: &str = "
        CREATE TABLE IF NOT EXISTS folder_settings (
            path TEXT PRIMARY KEY,
            sort_col TEXT,
            sort_reversed BOOLEAN,
            icon_size INTEGER,
            folders_first BOOLEAN
        );

        CREATE TABLE IF NOT EXISTS location_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            uri TEXT UNIQUE NOT NULL,
            timestamp INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS file_tags (
            path TEXT NOT NULL,
            tag TEXT NOT NULL,
            mtime INTEGER NOT NULL,
            PRIMARY KEY (path, tag)
        );
        CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag);

        CREATE TABLE IF NOT EXISTS folder_icons (
            path TEXT PRIMARY KEY,
            icon TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS quick_list (
            slot INTEGER PRIMARY KEY,
            path BLOB NOT NULL
        );
        CREATE TABLE IF NOT EXISTS quick_list_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            slot INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS quick_groups (
            pos INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            slot INTEGER
        );
        CREATE TABLE IF NOT EXISTS quick_group_items (
            pos INTEGER NOT NULL,
            slot INTEGER NOT NULL,
            path BLOB NOT NULL,
            PRIMARY KEY (pos, slot)
        );
        CREATE TABLE IF NOT EXISTS quick_group_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            selected INTEGER NOT NULL
        );
";

impl StateManager {
    pub fn new_with_path(db_path: &Path) -> Result<Self> {
        crate::hit!("new_with_path");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        }

        let conn = Connection::open(db_path)?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "mmap_size", 268435456)?;

        conn.execute_batch(SCHEMA)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn new() -> Result<Self> {
        crate::hit!("new");
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flux");

        Self::new_with_path(&data_dir.join("state.db"))
    }

    /// Throwaway database that lives only in memory. Used as a fallback when
    /// the on-disk state database cannot be opened, so flux still starts.
    pub fn new_in_memory() -> Result<Self> {
        crate::hit!("new_in_memory");
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}
