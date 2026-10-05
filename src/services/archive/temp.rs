//! Temporary file and scratch directory management for archive sessions.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static ARCHIVE_TEMP_FILES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static FLUX_TEMP_DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
static ACTIVE_TEMP_DIRS: Mutex<Vec<tempfile::TempDir>> = Mutex::new(Vec::new());

/// Registers an extracted directory guard so it stays alive while needed,
/// but can be cleared when browsing elsewhere.
pub fn register_temp_dir(dir: tempfile::TempDir) {
    if let Ok(mut lock) = ACTIVE_TEMP_DIRS.lock() {
        lock.push(dir);
    }
}

pub fn register_temp_file(path: PathBuf) {
    if let Ok(mut lock) = ARCHIVE_TEMP_FILES.lock() {
        lock.push(path);
    }
}

pub fn clear_archive_session_temp() {
    if let Ok(mut dirs) = ACTIVE_TEMP_DIRS.lock() {
        dirs.clear();
    }
    if let Ok(mut files) = ARCHIVE_TEMP_FILES.lock() {
        for path in files.drain(..) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

pub fn purge_stale_scratch_dirs() {
    let current_pid = std::process::id();
    let Ok(entries) = std::fs::read_dir("/tmp") else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(rest) = name.strip_prefix("flux-") else {
            continue;
        };
        let Some(pid_part) = rest.split('-').next() else {
            continue;
        };
        let Ok(pid) = pid_part.parse::<u32>() else {
            continue;
        };
        if pid == current_pid {
            continue;
        }
        let _ = std::fs::remove_dir_all(entry.path());
    }
}

/// Cleans up all extracted archive temp directories from `/tmp`.
#[allow(dead_code)]
pub fn clear_archive_temp_dirs() {
    if let Ok(mut lock) = ACTIVE_TEMP_DIRS.lock() {
        lock.clear();
    }
}

/// Returns a shared scratch folder for Flux that is automatically purged by RAII on exit.
pub fn flux_scratch_dir() -> &'static Path {
    FLUX_TEMP_DIR
        .get_or_init(|| {
            tempfile::Builder::new()
                .prefix(&format!("flux-{}-", std::process::id()))
                .tempdir()
                .expect("failed to create flux temp scratch dir")
        })
        .path()
}
