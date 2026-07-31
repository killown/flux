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

            CREATE TABLE IF NOT EXISTS location_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uri TEXT UNIQUE NOT NULL,
                timestamp INTEGER NOT NULL
            );
        ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Creates a StateManager with a specific database path (useful for testing).
    #[cfg(test)]
    pub fn new_with_path(db_path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        }

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

            CREATE TABLE IF NOT EXISTS location_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uri TEXT UNIQUE NOT NULL,
                timestamp INTEGER NOT NULL
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

    /// Adds or updates a URI in location history, keeping the maximum size capped at 10000.
    pub fn add_location(&self, uri: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO location_history (uri, timestamp) VALUES (?1, ?2)
             ON CONFLICT(uri) DO UPDATE SET timestamp = ?2",
            params![uri, now],
        )?;

        conn.execute(
            "DELETE FROM location_history WHERE id NOT IN (
                SELECT id FROM location_history ORDER BY timestamp DESC LIMIT 10000
            )",
            [],
        )?;

        Ok(())
    }

    /// Deletes a specific URI from the location history.
    pub fn remove_location(&self, uri: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM location_history WHERE uri = ?1", params![uri])?;
        Ok(())
    }

    /// Retrieves location history ordered by most recent first.
    #[allow(dead_code)]
    pub fn get_location_history(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT uri FROM location_history ORDER BY timestamp DESC")?;

        let iter = stmt.query_map([], |row| row.get(0))?;
        let mut history = Vec::new();
        for uri in iter {
            history.push(uri?);
        }
        Ok(history)
    }

    /// Clears all entries from the location history.
    pub fn clear_location_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM location_history", [])?;
        Ok(())
    }
}

impl std::fmt::Debug for StateManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateManager").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_db() -> (StateManager, tempfile::TempDir) {
        // Create a temporary directory that will be deleted when dropped
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_state.db");

        let manager = StateManager::new_with_path(&db_path).expect("Failed to create test DB");
        (manager, temp_dir)
    }

    #[test]
    fn test_save_and_get_view() {
        let (manager, _temp_dir) = create_test_db();
        let path = PathBuf::from("/home/user/downloads");

        manager.save_view(&path, "Date", true, 64, false).unwrap();

        let result = manager.get_view(&path).unwrap().unwrap();

        assert_eq!(result.0, "Date");
        assert!(result.1);
        assert_eq!(result.2, 64);
        assert!(!result.3);
    }

    #[test]
    fn test_get_nonexistent_view() {
        let (manager, _temp_dir) = create_test_db();
        let path = PathBuf::from("/nonexistent/path");

        let result = manager.get_view(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_existing_view() {
        let (manager, _temp_dir) = create_test_db();
        let path = PathBuf::from("/home/user/Downloads");

        manager.save_view(&path, "Date", true, 64, false).unwrap();

        manager.save_view(&path, "Size", false, 256, true).unwrap();

        let result = manager.get_view(&path).unwrap().unwrap();
        assert_eq!(result.0, "Size");
        assert_eq!(result.1, false);
        assert_eq!(result.2, 256);
        assert_eq!(result.3, true);
    }

    #[test]
    fn test_rename_path() {
        let (manager, _temp_dir) = create_test_db();
        let old_path = PathBuf::from("/home/user/OldName");
        let new_path = PathBuf::from("/home/user/NewName");

        manager
            .save_view(&old_path, "Name", false, 128, true)
            .unwrap();

        manager.rename_path(&old_path, &new_path).unwrap();

        assert!(manager.get_view(&old_path).unwrap().is_none());

        let result = manager.get_view(&new_path).unwrap().unwrap();
        assert_eq!(result.0, "Name");
    }

    #[test]
    fn test_scrub_orphans() {
        let (manager, temp_dir) = create_test_db();

        let real_dir = temp_dir.path().join("real_folder");
        std::fs::create_dir(&real_dir).unwrap();

        let fake_dir = temp_dir.path().join("fake_folder");

        manager
            .save_view(&real_dir, "Name", false, 128, true)
            .unwrap();
        manager
            .save_view(&fake_dir, "Date", true, 64, false)
            .unwrap();

        assert!(manager.get_view(&real_dir).unwrap().is_some());
        assert!(manager.get_view(&fake_dir).unwrap().is_some());

        manager.scrub_orphans().unwrap();

        assert!(manager.get_view(&real_dir).unwrap().is_some());

        assert!(manager.get_view(&fake_dir).unwrap().is_none());
    }

    #[test]
    fn test_location_history() {
        let (manager, _temp_dir) = create_test_db();

        manager.add_location("smb://server/share").unwrap();
        manager.add_location("sftp://localhost").unwrap();
        manager.add_location("smb://server/share").unwrap(); // duplicate update test

        let history = manager.get_location_history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], "smb://server/share"); // most recent first

        manager.clear_location_history().unwrap();
        let empty_history = manager.get_location_history().unwrap();
        assert!(empty_history.is_empty());
    }
}
