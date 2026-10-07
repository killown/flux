use super::StateManager;
use rusqlite::{params, Result};

impl StateManager {
    pub fn add_location(&self, uri: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_micros();

        conn.execute(
            "INSERT INTO location_history (uri, timestamp) VALUES (?1, ?2)
                 ON CONFLICT(uri) DO UPDATE SET timestamp = ?2",
            params![uri, now],
        )?;

        conn.execute(
            "DELETE FROM location_history WHERE id NOT IN (
                    SELECT id FROM location_history ORDER BY timestamp DESC, id DESC LIMIT 10000
                )",
            [],
        )?;

        Ok(())
    }

    pub fn remove_location(&self, uri: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM location_history WHERE uri = ?1", params![uri])?;
        Ok(())
    }

    pub fn get_location_history(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT uri FROM location_history ORDER BY timestamp DESC, id DESC")?;

        let iter = stmt.query_map([], |row| row.get(0))?;
        let mut history = Vec::new();
        for uri in iter {
            history.push(uri?);
        }
        Ok(history)
    }

    pub fn clear_location_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM location_history", [])?;
        Ok(())
    }
}
