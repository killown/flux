use super::StateManager;
use crate::model::quick_groups::{QuickGroup, QuickGroups};
use rusqlite::{params, Connection, Result};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

/// Reads every saved list, or `None` when none were ever saved.
fn read_groups(conn: &Connection) -> Option<QuickGroups> {
    let mut stmt = conn
        .prepare("SELECT pos, name, slot FROM quick_groups ORDER BY pos")
        .ok()?;
    let heads: Vec<(i64, String, Option<i64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .ok()?
        .flatten()
        .collect();
    if heads.is_empty() {
        return None;
    }

    let mut item_stmt = conn
        .prepare("SELECT path FROM quick_group_items WHERE pos = ?1 ORDER BY slot")
        .ok()?;
    let mut lists = Vec::with_capacity(heads.len());
    for (pos, name, slot) in heads {
        let items: Vec<PathBuf> = item_stmt
            .query_map(params![pos], |r| r.get::<_, Vec<u8>>(0))
            .ok()?
            .flatten()
            .map(|b| PathBuf::from(OsString::from_vec(b)))
            .collect();
        lists.push(QuickGroup {
            name,
            items,
            index: slot.and_then(|s| usize::try_from(s).ok()),
        });
    }

    let selected = conn
        .query_row(
            "SELECT selected FROM quick_group_state WHERE id = 1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .ok()
        .and_then(|s| usize::try_from(s).ok())
        .unwrap_or(0);

    Some(QuickGroups { lists, selected })
}

impl StateManager {
    /// Saved Quick Lists, repaired so that invariant G holds.
    ///
    /// A database written before named lists existed has only the flat list.
    /// It comes back as a single list called `default_name`. Nothing is
    /// deleted from the old table, the next save simply writes the new ones.
    pub fn load_quick_groups(&self, default_name: &str) -> QuickGroups {
        crate::hit!("load_quick_groups");
        // The guard is dropped before `load_quick_list` locks the same mutex.
        let saved = {
            let conn = self.conn.lock().unwrap();
            read_groups(&conn)
        };
        let mut groups = match saved {
            Some(g) => g,
            None => {
                let (items, slot) = self.load_quick_list();
                QuickGroups::single(default_name, items, slot)
            }
        };
        groups.normalize();
        groups
    }

    /// Replaces every saved list and the selection in one transaction.
    pub fn save_quick_groups(&self, groups: &QuickGroups) -> Result<()> {
        crate::hit!("save_quick_groups");
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM quick_groups", [])?;
        tx.execute("DELETE FROM quick_group_items", [])?;
        {
            let mut head =
                tx.prepare("INSERT INTO quick_groups (pos, name, slot) VALUES (?1, ?2, ?3)")?;
            let mut item =
                tx.prepare("INSERT INTO quick_group_items (pos, slot, path) VALUES (?1, ?2, ?3)")?;
            for (pos, g) in groups.lists.iter().enumerate() {
                head.execute(params![pos as i64, g.name, g.index.map(|i| i as i64)])?;
                for (slot, p) in g.items.iter().enumerate() {
                    item.execute(params![pos as i64, slot as i64, p.as_os_str().as_bytes()])?;
                }
            }
        }
        tx.execute(
            "INSERT INTO quick_group_state (id, selected) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET selected = excluded.selected",
            params![groups.selected as i64],
        )?;
        tx.commit()
    }
}
