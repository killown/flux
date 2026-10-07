use super::StateManager;
use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

impl StateManager {
    #[allow(dead_code)]
    pub fn new_with_path(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        }

        let conn = Connection::open(db_path)?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "mmap_size", 268435456)?;

        conn.execute_batch(
            "
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
                ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flux");

        std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
        let db_path = data_dir.join("state.db");

        let conn = Connection::open(db_path)?;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "mmap_size", 268435456)?;

        conn.execute_batch(
            "
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
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}
