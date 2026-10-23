//! Batched writer for the in-archive thumbnail cache (zip only).
//!

use super::backends::write_thumbnails_to_archive;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Number of queued thumbnails (across all archives) that triggers an automatic flush.
const FLUSH_THRESHOLD: usize = 32;

/// Thumbnails waiting to be written into one archive.
struct PendingArchive {
    /// `(inner_path, png_bytes)` pairs, at most one per `inner_path`.
    thumbs: Vec<(String, Vec<u8>)>,
    /// Password forwarded to the writer (skips encrypted archives).
    password: Option<String>,
}

/// Queue of thumbnails not yet committed, keyed by archive path.
static PENDING: OnceLock<Mutex<HashMap<PathBuf, PendingArchive>>> = OnceLock::new();

/// True while a threshold-triggered flush thread is spawned but has not started draining.
static FLUSH_SCHEDULED: AtomicBool = AtomicBool::new(false);

/// False once the user disables the cache: nothing is queued or written.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Serialises archive rewrites so two flushes never rebuild the same zip concurrently.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Returns the shared pending queue, creating it on first use.
fn pending() -> &'static Mutex<HashMap<PathBuf, PendingArchive>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Enables or disables the cache, disabling also discards everything still queued.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
    if !on {
        pending().lock().clear();
    }
}

/// Queues a PNG thumbnail for `inner_path` to be stored inside `archive`.
pub fn enqueue(archive: PathBuf, inner_path: String, png_bytes: Vec<u8>, password: Option<String>) {
    crate::hit!("enqueue");
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    let total = {
        let mut map = pending().lock();
        let entry = map.entry(archive).or_insert_with(|| PendingArchive {
            thumbs: Vec::new(),
            password: None,
        });
        if entry.password.is_none() {
            entry.password = password;
        }
        match entry.thumbs.iter_mut().find(|(p, _)| *p == inner_path) {
            Some(slot) => slot.1 = png_bytes,
            None => entry.thumbs.push((inner_path, png_bytes)),
        }
        map.values().map(|a| a.thumbs.len()).sum::<usize>()
    };

    if total >= FLUSH_THRESHOLD && !FLUSH_SCHEDULED.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            FLUSH_SCHEDULED.store(false, Ordering::SeqCst);
            flush_all();
        });
    }
}

/// Commits every queued thumbnail, rewriting each affected archive once.
///
/// Returns immediately: the queue is drained under the lock and each archive is
/// written on its own thread. Cheap when the queue is empty.
pub fn flush_all() {
    crate::hit!("flush_all");
    let drained: Vec<(PathBuf, PendingArchive)> = {
        let mut map = pending().lock();
        if map.is_empty() {
            return;
        }
        map.drain().collect()
    };

    for (archive, batch) in drained {
        std::thread::spawn(move || write_batch(&archive, batch));
    }
}

/// Writes one archive's batch, logging (never panicking on) failures.
fn write_batch(archive: &Path, batch: PendingArchive) {
    crate::hit!("write_batch");
    if batch.thumbs.is_empty() || !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    let _guard = WRITE_LOCK.lock();
    if let Err(e) = write_thumbnails_to_archive(archive, &batch.thumbs, batch.password.as_deref()) {
        eprintln!("[thumb-cache] {}: {e}", archive.display());
    }
}
