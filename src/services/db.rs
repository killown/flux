use rusqlite::{params, Connection, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Persistent state manager for flux.
pub struct StateManager {
    conn: Mutex<Connection>,
}

impl StateManager {
    /// Deletes all occurrences of a tag globally from the database.
    pub fn delete_tag_globally(&self, tag: &str) -> Result<()> {
        let clean = tag.trim().trim_start_matches('#');
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM file_tags WHERE tag = ?1", params![clean])?;
        Ok(())
    }

    /// Creates a StateManager with a specific database path (useful for testing).
    #[allow(dead_code)]
    pub fn new_with_path(db_path: &Path) -> Result<Self> {
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

            CREATE TABLE IF NOT EXISTS file_tags (
                path TEXT NOT NULL,
                tag TEXT NOT NULL,
                mtime INTEGER NOT NULL,
                PRIMARY KEY (path, tag)
            );
            CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

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

            CREATE TABLE IF NOT EXISTS file_tags (
                path TEXT NOT NULL,
                tag TEXT NOT NULL,
                mtime INTEGER NOT NULL,
                PRIMARY KEY (path, tag)
            );
            CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag);
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

        conn.execute(
            "UPDATE file_tags SET path = ?1 WHERE path = ?2",
            params![new_str, old_str],
        )?;
        Ok(())
    }

    /// Updates cached tags for a given path.
    pub fn set_tags(&self, path: &Path, tags: &[String], mtime: i64) -> Result<()> {
        let path_str = path.to_string_lossy();
        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.transaction()?;

        tx.execute("DELETE FROM file_tags WHERE path = ?1", params![path_str])?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO file_tags (path, tag, mtime) VALUES (?1, ?2, ?3)
                 ON CONFLICT(path, tag) DO UPDATE SET mtime = excluded.mtime",
            )?;

            for tag in tags {
                let clean = tag.trim().trim_start_matches('#');
                if !clean.is_empty() {
                    stmt.execute(params![path_str, clean, mtime])?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Retrieves cached tags for a given path.
    #[allow(dead_code)]
    pub fn get_tags(&self, path: &Path) -> Result<Vec<String>> {
        let path_str = path.to_string_lossy();
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT tag FROM file_tags WHERE path = ?1 ORDER BY tag ASC")?;
        let rows = stmt.query_map(params![path_str], |row| row.get(0))?;

        let mut tags = Vec::new();
        for tag in rows {
            tags.push(tag?);
        }
        Ok(tags)
    }

    /// Retrieves all file paths associated with a specific tag.
    pub fn get_paths_for_tag(&self, tag: &str) -> Result<Vec<PathBuf>> {
        let clean = tag.trim().trim_start_matches('#');
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT path FROM file_tags WHERE tag = ?1 ORDER BY path ASC")?;
        let rows = stmt.query_map(params![clean], |row| row.get::<_, String>(0))?;

        let mut paths = Vec::new();
        for path_str in rows {
            paths.push(PathBuf::from(path_str?));
        }
        Ok(paths)
    }

    /// Retrieves all unique tags stored in the database.
    pub fn list_all_tags(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT tag FROM file_tags ORDER BY tag ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;

        let mut tags = Vec::new();
        for tag in rows {
            tags.push(tag?);
        }
        Ok(tags)
    }

    /// Removes entries from the database if the corresponding filesystem paths no longer exist.
    pub fn scrub_orphans(&self) -> Result<()> {
        let (paths, tag_paths): (Vec<String>, Vec<String>) = {
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            let mut stmt = conn.prepare("SELECT path FROM folder_settings")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let p1 = rows.flatten().collect();

            let mut tag_stmt = conn.prepare("SELECT DISTINCT path FROM file_tags")?;
            let tag_rows = tag_stmt.query_map([], |row| row.get::<_, String>(0))?;
            let p2 = tag_rows.flatten().collect();

            (p1, p2)
        };

        let mut orphans = Vec::new();
        for path_str in paths {
            // Avoid deleting non-filesystem virtual URIs like trash:/// or /archive://
            if path_str.starts_with("trash://")
                || path_str.starts_with("/archive://")
                || path_str.starts_with("recent://")
                || path_str.contains("://")
            {
                continue;
            }

            if !std::path::Path::new(&path_str).exists() {
                orphans.push(path_str);
            }
        }

        let mut tag_orphans = Vec::new();
        for path_str in tag_paths {
            if !std::path::Path::new(&path_str).exists() {
                tag_orphans.push(path_str);
            }
        }

        if !orphans.is_empty() || !tag_orphans.is_empty() {
            let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            let tx = conn.transaction()?;
            {
                if !orphans.is_empty() {
                    let mut del_stmt = tx.prepare("DELETE FROM folder_settings WHERE path = ?1")?;
                    for orphan in &orphans {
                        let _ = del_stmt.execute(params![orphan]);
                    }
                }
                if !tag_orphans.is_empty() {
                    let mut del_tag_stmt = tx.prepare("DELETE FROM file_tags WHERE path = ?1")?;
                    for orphan in &tag_orphans {
                        let _ = del_tag_stmt.execute(params![orphan]);
                    }
                }
            }
            tx.commit()?;
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
