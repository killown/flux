use super::StateManager;
use rayon::prelude::*;
use rusqlite::{params, Result};

impl StateManager {
    pub fn scrub_orphans(&self) -> Result<()> {
        let (paths, tag_paths, icon_paths): (Vec<String>, Vec<String>, Vec<String>) = {
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            let mut stmt = conn.prepare("SELECT path FROM folder_settings")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let p1 = rows.flatten().collect();

            let mut tag_stmt = conn.prepare("SELECT DISTINCT path FROM file_tags")?;
            let tag_rows = tag_stmt.query_map([], |row| row.get::<_, String>(0))?;
            let p2 = tag_rows.flatten().collect();

            let mut icon_stmt = conn.prepare("SELECT path FROM folder_icons")?;
            let icon_rows = icon_stmt.query_map([], |row| row.get::<_, String>(0))?;
            let p3 = icon_rows.flatten().collect();

            (p1, p2, p3)
        };

        let mounts = crate::services::mounts::MountTable::load();
        let is_orphan = |path_str: &String| -> bool {
            if path_str.contains("://") {
                // trash://, recent://, smb://, /archive:// ... are not local paths
                return false;
            }
            crate::services::mounts::is_confirmed_missing(std::path::Path::new(path_str), &mounts)
        };

        let orphans: Vec<String> = paths.into_par_iter().filter(|p| is_orphan(p)).collect();

        let tag_orphans: Vec<String> = tag_paths.into_par_iter().filter(|p| is_orphan(p)).collect();

        let icon_orphans: Vec<String> = icon_paths
            .into_par_iter()
            .filter(|p| is_orphan(p))
            .collect();

        if !orphans.is_empty() || !tag_orphans.is_empty() || !icon_orphans.is_empty() {
            {
                let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
                let tx = conn.transaction()?;
                {
                    if !orphans.is_empty() {
                        let mut stmt = tx.prepare("DELETE FROM folder_settings WHERE path = ?1")?;
                        for orphan in &orphans {
                            let _ = stmt.execute(params![orphan]);
                        }
                    }
                    if !tag_orphans.is_empty() {
                        let mut stmt = tx.prepare("DELETE FROM file_tags WHERE path = ?1")?;
                        for orphan in &tag_orphans {
                            let _ = stmt.execute(params![orphan]);
                        }
                    }
                    if !icon_orphans.is_empty() {
                        let mut stmt = tx.prepare("DELETE FROM folder_icons WHERE path = ?1")?;
                        for orphan in &icon_orphans {
                            let _ = stmt.execute(params![orphan]);
                        }
                    }
                }
                tx.commit()?;
            }

            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            let _ = conn.execute("VACUUM", []);
        }

        Ok(())
    }
}
