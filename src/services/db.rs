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
        let path = PathBuf::from("/home/user/Documents");

        // Save view settings
        manager.save_view(&path, "Name", false, 128, true).unwrap();

        // Retrieve view settings
        let result = manager.get_view(&path).unwrap();
        assert!(result.is_some());

        let (sort_col, reversed, icon_size, folders_first) = result.unwrap();
        assert_eq!(sort_col, "Name");
        assert_eq!(reversed, false);
        assert_eq!(icon_size, 128);
        assert_eq!(folders_first, true);
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

        // Save initial settings
        manager.save_view(&path, "Date", true, 64, false).unwrap();

        // Update settings
        manager.save_view(&path, "Size", false, 256, true).unwrap();

        // Verify update
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

        // Save with old path
        manager
            .save_view(&old_path, "Name", false, 128, true)
            .unwrap();

        // Rename path
        manager.rename_path(&old_path, &new_path).unwrap();

        // Old path should no longer exist
        assert!(manager.get_view(&old_path).unwrap().is_none());

        // New path should exist with same data
        let result = manager.get_view(&new_path).unwrap().unwrap();
        assert_eq!(result.0, "Name");
    }

    #[test]
    fn test_scrub_orphans() {
        let (manager, temp_dir) = create_test_db();

        // Create a real directory
        let real_dir = temp_dir.path().join("real_folder");
        std::fs::create_dir(&real_dir).unwrap();

        // Create a path that does not exist
        let fake_dir = temp_dir.path().join("fake_folder");

        // Save both to DB
        manager
            .save_view(&real_dir, "Name", false, 128, true)
            .unwrap();
        manager
            .save_view(&fake_dir, "Date", true, 64, false)
            .unwrap();

        // Verify both exist in DB
        assert!(manager.get_view(&real_dir).unwrap().is_some());
        assert!(manager.get_view(&fake_dir).unwrap().is_some());

        // Scrub orphans
        manager.scrub_orphans().unwrap();

        // Real directory should still be there
        assert!(manager.get_view(&real_dir).unwrap().is_some());

        // Fake directory should be removed
        assert!(manager.get_view(&fake_dir).unwrap().is_none());
    }
}
