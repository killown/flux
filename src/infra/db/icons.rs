use super::StateManager;
use rusqlite::{params, Result};

impl StateManager {
    pub fn load_folder_icons(&self) -> std::collections::HashMap<String, String> {
        crate::hit!("load_folder_icons");
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
        crate::hit!("set_folder_icon");
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO folder_icons (path, icon) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET icon = excluded.icon",
            params![path, icon],
        )?;
        Ok(())
    }

    pub fn remove_folder_icon(&self, path: &str) -> Result<()> {
        crate::hit!("remove_folder_icon");
        let clean = path.trim_end_matches('/');
        let with_slash = format!("{}/", clean);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM folder_icons WHERE path = ?1 OR path = ?2",
            params![clean, with_slash],
        )?;
        Ok(())
    }
}
