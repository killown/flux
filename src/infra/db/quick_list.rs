use super::StateManager;
use rusqlite::{params, Result};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

impl StateManager {
    /// Saved Quick List in slot order, plus the saved current slot.
    pub fn load_quick_list(&self) -> (Vec<PathBuf>, Option<usize>) {
        crate::hit!("load_quick_list");
        let conn = self.conn.lock().unwrap();
        let items: Vec<PathBuf> = match conn.prepare("SELECT path FROM quick_list ORDER BY slot") {
            Ok(mut stmt) => match stmt.query_map([], |row| row.get::<_, Vec<u8>>(0)) {
                Ok(rows) => rows
                    .flatten()
                    .map(|b| PathBuf::from(OsString::from_vec(b)))
                    .collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        let slot = conn
            .query_row("SELECT slot FROM quick_list_state WHERE id = 1", [], |r| {
                r.get::<_, i64>(0)
            })
            .ok()
            .and_then(|s| usize::try_from(s).ok());
        (items, slot)
    }

    /// Replaces the saved list and slot in one transaction.
    ///
    /// Superseded by `save_quick_groups` in the app, kept for the legacy
    /// table and its tests.
    #[allow(dead_code)]
    pub fn save_quick_list(&self, items: &[PathBuf], slot: Option<usize>) -> Result<()> {
        crate::hit!("save_quick_list");
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM quick_list", [])?;
        {
            let mut ins = tx.prepare("INSERT INTO quick_list (slot, path) VALUES (?1, ?2)")?;
            for (i, p) in items.iter().enumerate() {
                ins.execute(params![i as i64, p.as_os_str().as_bytes()])?;
            }
        }
        match slot {
            Some(s) => tx.execute(
                "INSERT INTO quick_list_state (id, slot) VALUES (1, ?1)
                 ON CONFLICT(id) DO UPDATE SET slot = excluded.slot",
                params![s as i64],
            )?,
            None => tx.execute("DELETE FROM quick_list_state", [])?,
        };
        tx.commit()
    }
}
