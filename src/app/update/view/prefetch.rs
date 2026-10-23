//! Neighbour prefetch for the peek-preview panel.
//!
//! When the preview panel is visible and the user selects a file, this
//! module dispatches a background `compute_preview` for the file one slot
//! before and one slot after the selection. Results live in a small LRU
//! keyed on `(path, mtime)`. The LRU is consulted by `handle_preview_load`,
//! on a hit, the panel is populated without touching the blocking pool.

use crate::model::{AppMsg, FluxApp};
use crate::ui::panels::preview::PreviewContent;
use parking_lot::Mutex;
use relm4::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

const CACHE_CAP: usize = 24;
const PREFETCH_RADIUS: u32 = 1;

/// Bounded LRU keyed on `(path, mtime_secs)`.
pub struct PreviewCache {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for PreviewCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewCache").finish()
    }
}

struct Inner {
    map: HashMap<(PathBuf, i64), Arc<PreviewContent>>,
    clock: u64,
    age: HashMap<(PathBuf, i64), u64>,
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                clock: 0,
                age: HashMap::new(),
            }),
        }
    }
}

impl PreviewCache {
    pub fn get(&self, path: &Path, mtime: i64) -> Option<Arc<PreviewContent>> {
        let mut inner = self.inner.lock();
        let key = (path.to_path_buf(), mtime);
        let hit = inner.map.get(&key).cloned();
        if hit.is_some() {
            let tick = inner.clock.wrapping_add(1);
            inner.clock = tick;
            inner.age.insert(key, tick);
        }
        hit
    }

    pub fn insert(&self, path: PathBuf, mtime: i64, content: PreviewContent) {
        crate::hit!("insert");
        let mut inner = self.inner.lock();
        let key = (path, mtime);
        let tick = inner.clock.wrapping_add(1);
        inner.clock = tick;
        inner.map.insert(key.clone(), Arc::new(content));
        inner.age.insert(key, tick);

        while inner.map.len() > CACHE_CAP {
            let oldest = inner
                .age
                .iter()
                .min_by_key(|(_, &t)| t)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                inner.map.remove(&k);
                inner.age.remove(&k);
            } else {
                break;
            }
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.map.clear();
        inner.age.clear();
    }
}

/// Stat mtime for the LRU key. `0` when unavailable, still a valid key.
fn mtime_of(path: &Path) -> i64 {
    crate::hit!("mtime_of");
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl FluxApp {
    pub fn handle_preview_prefetch(
        &self,
        path: PathBuf,
        session: u64,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_preview_prefetch");
        if !self.preview_panel_visible {
            return;
        }
        if session != self.preview_session.load(Ordering::SeqCst) {
            return;
        }
        let neighbours = self.preview_neighbour_paths(&path);
        if neighbours.is_empty() {
            return;
        }
        let cache = self.preview_cache.clone();
        let session_arc = self.preview_session.clone();
        let sender_clone = sender.clone();
        relm4::spawn_blocking(move || {
            for neighbour in neighbours {
                if session_arc.load(Ordering::SeqCst) != session {
                    return;
                }
                let mt = mtime_of(&neighbour);
                if cache.get(&neighbour, mt).is_some() {
                    continue;
                }
                let content = crate::app::update::view::preview::compute_preview(&neighbour);
                cache.insert(neighbour.clone(), mt, content.clone());
                let _ = sender_clone.input_sender().send(AppMsg::PreviewPrefetched {
                    path: neighbour,
                    content: Arc::new(content),
                });
            }
        });
    }

    /// Returns up to `2 * PREFETCH_RADIUS` neighbour paths around `path`
    /// inside the active tab's file list.
    fn preview_neighbour_paths(&self, path: &PathBuf) -> Vec<PathBuf> {
        crate::hit!("preview_neighbour_paths");
        let Some(tab) = self.tabs.get(self.active_tab_index) else {
            return Vec::new();
        };
        let files = &tab.files;
        let Some(here) =
            (0..files.len()).find(|&i| files.get(i).is_some_and(|w| w.borrow().path == *path))
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for d in 1..=PREFETCH_RADIUS {
            if let Some(i) = here.checked_sub(d) {
                if let Some(w) = files.get(i) {
                    out.push(w.borrow().path.clone());
                }
            }
            let j = here + d;
            if let Some(w) = files.get(j) {
                out.push(w.borrow().path.clone());
            }
        }
        out
    }
}
