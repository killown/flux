use super::StateManager;
use rusqlite::{params, Result};
use std::path::Path;

impl StateManager {
    pub fn save_view(
        &self,
        path: &Path,
        sort_col: &str,
        reversed: bool,
        icon_size: u32,
        folders_first: bool,
    ) -> Result<()> {
        crate::hit!("save_view");
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

    pub fn get_view(&self, path: &Path) -> Result<Option<(String, bool, u32, bool)>> {
        crate::hit!("get_view");
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

    pub fn rename_path(&self, old_path: &Path, new_path: &Path) -> Result<()> {
        let old_lossy = old_path.to_string_lossy();
        let new_lossy = new_path.to_string_lossy();
        let old_str = old_lossy.trim_end_matches('/');
        let new_str = new_lossy.trim_end_matches('/');
        if old_str.is_empty() || new_str.is_empty() || old_str == new_str {
            return Ok(());
        }

        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.transaction()?;
        for table in ["folder_settings", "file_tags", "folder_icons"] {
            tx.execute(
                &format!(
                    "UPDATE OR REPLACE {table}
                         SET path = ?2 || substr(path, length(?1) + 1)
                         WHERE path = ?1
                            OR substr(path, 1, length(?1) + 1) = ?1 || '/'"
                ),
                params![old_str, new_str],
            )?;
        }
        tx.commit()
    }
}
