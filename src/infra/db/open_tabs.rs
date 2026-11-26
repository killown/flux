use super::StateManager;
use rusqlite::{params, Result};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

impl StateManager {
    /// Returns the saved tab paths in tab order and the index of the active tab.
    /// Empty when no tabs were ever saved.
    pub fn load_open_tabs(&self) -> (Vec<PathBuf>, usize) {
        crate::hit!("load_open_tabs");
        let conn = self.conn.lock().unwrap();
        let paths: Vec<PathBuf> = conn
            .prepare("SELECT path FROM open_tabs ORDER BY pos")
            .and_then(|mut stmt| {
                stmt.query_map([], |r| r.get::<_, Vec<u8>>(0)).map(|rows| {
                    rows.flatten()
                        .map(|b| PathBuf::from(OsString::from_vec(b)))
                        .collect()
                })
            })
            .unwrap_or_default();
        let active = conn
            .query_row("SELECT active FROM open_tabs_state WHERE id = 1", [], |r| {
                r.get::<_, i64>(0)
            })
            .ok()
            .and_then(|a| usize::try_from(a).ok())
            .unwrap_or(0);
        (paths, active)
    }

    /// Replaces the saved tabs with `paths`, in tab order, and records `active`.
    pub fn save_open_tabs(&self, paths: &[PathBuf], active: usize) -> Result<()> {
        crate::hit!("save_open_tabs");
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM open_tabs", [])?;
        {
            let mut insert = tx.prepare("INSERT INTO open_tabs (pos, path) VALUES (?1, ?2)")?;
            for (pos, path) in paths.iter().enumerate() {
                insert.execute(params![pos as i64, path.as_os_str().as_bytes()])?;
            }
        }
        tx.execute(
            "INSERT INTO open_tabs_state (id, active) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET active = excluded.active",
            params![active as i64],
        )?;
        tx.commit()
    }
}
