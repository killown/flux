use super::StateManager;
use rusqlite::{params, Result};
use std::path::{Path, PathBuf};

impl StateManager {
    pub fn delete_tag_globally(&self, tag: &str) -> Result<()> {
        let clean = tag.trim().trim_start_matches('#');
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM file_tags WHERE tag = ?1", params![clean])?;
        Ok(())
    }

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

    #[allow(dead_code)]
    pub fn get_tags(&self, path: &Path) -> Result<Vec<String>> {
        crate::hit!("get_tags");
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
}
