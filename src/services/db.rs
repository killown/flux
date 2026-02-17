use rusqlite::{params, Connection, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Persistent state manager for flux.
pub struct StateManager {
    conn: Mutex<Connection>,
}

impl StateManager {
    /// Initializes the database in the user's local data directory.
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flux");

        std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
        let db_path = data_dir.join("state.db");

        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS folder_settings (
                path TEXT PRIMARY KEY,
                sort_col TEXT,
                sort_reversed BOOLEAN,
                icon_size INTEGER,
                folders_first BOOLEAN
            );
        ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Persists view settings for a specific directory.
    pub fn save_view(
        &self,
        path: &Path,
        sort_col: &str,
        reversed: bool,
        icon_size: u32,
        folders_first: bool,
    ) -> Result<()> {
        let path_str = path.to_string_lossy();
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO folder_settings (path, sort_col, sort_reversed, icon_size, folders_first)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                sort_col = excluded.sort_col,
                sort_reversed = excluded.sort_reversed,
                icon_size = excluded.icon_size,
                folders_first = excluded.folders_first",
            params![path_str, sort_col, reversed, icon_size, folders_first],
        )?;
        Ok(())
    }

    /// Retrieves saved view settings for the given path.
    pub fn get_view(&self, path: &Path) -> Result<Option<(String, bool, u32, bool)>> {
        let path_str = path.to_string_lossy();
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT sort_col, sort_reversed, icon_size, folders_first FROM folder_settings WHERE path = ?1"
        )?;

        let mut rows = stmt.query(params![path_str])?;

        if let Some(row) = rows.next()? {
            Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        } else {
            Ok(None)
        }
    }

    /// Updates the path key when a directory is renamed.
    pub fn rename_path(&self, old_path: &Path, new_path: &Path) -> Result<()> {
        let old_str = old_path.to_string_lossy();
        let new_str = new_path.to_string_lossy();
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE folder_settings SET path = ?1 WHERE path = ?2",
            params![new_str, old_str],
        )?;
        Ok(())
    }

    /// Removes entries from the database if the corresponding filesystem paths no longer exist.
    pub fn scrub_orphans(&self) -> Result<()> {
        let paths: Vec<String> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT path FROM folder_settings")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.flatten().collect()
        };

        let mut orphans = Vec::new();
        for path_str in paths {
            if !std::path::Path::new(&path_str).exists() {
                orphans.push(path_str);
            }
        }

        if !orphans.is_empty() {
            let conn = self.conn.lock().unwrap();
            let mut del_stmt = conn.prepare("DELETE FROM folder_settings WHERE path = ?1")?;
            for orphan in orphans {
                let _ = del_stmt.execute(params![orphan]);
            }
            let _ = conn.execute("VACUUM", []);
        }

        Ok(())
    }
}

impl std::fmt::Debug for StateManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateManager").finish()
    }
}
