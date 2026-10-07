use super::StateManager;
use rusqlite::{params, Result};

impl StateManager {
    pub fn load_folder_icons(&self) -> std::collections::HashMap<String, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare("SELECT path, icon FROM folder_icons") {
            Ok(s) => s,
            Err(_) => return std::collections::HashMap::new(),
        };
        let rows = match stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            Ok(r) => r,
            Err(_) => return std::collections::HashMap::new(),
        };
        rows.flatten().collect()
    }

    pub fn set_folder_icon(&self, path: &str, icon: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO folder_icons (path, icon) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET icon = excluded.icon",
            params![path, icon],
        )?;
        Ok(())
    }

    pub fn remove_folder_icon(&self, path: &str) -> Result<()> {
        let clean = path.trim_end_matches('/');
        let with_slash = format!("{}/", clean);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM folder_icons WHERE path = ?1 OR path = ?2",
            params![clean, with_slash],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn rename_folder_icon(&self, old_path: &str, new_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE folder_icons SET path = ?1 WHERE path = ?2",
            params![new_path, old_path],
        )?;
        Ok(())
    }
}
