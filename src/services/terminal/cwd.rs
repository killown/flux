//! Publishes the directory shown in the grid to `$XDG_CACHE_HOME/flux/cwd`
//! and applies `goto` requests written by a shell (see `shell_functions.rs`).
//!
//! * `cwd`  - the directory of the window that navigated last.
//! * `pid`  - that window's process id, so `flux_goto` can tell a live
//!   window from a stale file, and so only that window answers a `goto`.
//! * `goto` - written by a shell; the owning window navigates to it.

use gtk::gio;
use gtk::prelude::*;
use std::cell::OnceCell;
use std::path::{Path, PathBuf};

thread_local! {
    static GOTO_MONITOR: OnceCell<gio::FileMonitor> = const { OnceCell::new() };
}

fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("flux"))
}

/// Replaces `path` atomically so a shell never reads a half-written file.
fn write_atomic(path: &Path, data: &str) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, data).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Records `path` as the grid's directory. Virtual locations (`trash:///`,
/// `recent:///`, archives, network URIs) are skipped: a shell cannot `cd`
/// into them, so the last real directory stays published.
pub(crate) fn write_cwd_cache(path: &Path) {
    let text = path.to_string_lossy();
    if !path.is_absolute() || text.contains("://") {
        return;
    }
    let Some(dir) = cache_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    write_atomic(&dir.join("cwd"), &format!("{text}\n"));
    write_atomic(&dir.join("pid"), &format!("{}\n", std::process::id()));
}

/// Watches `$XDG_CACHE_HOME/flux/goto` and navigates when a shell writes a
/// directory to it. Idempotent for the lifetime of the process.
#[allow(dead_code)]
pub(crate) fn setup_goto_watcher() {
    let Some(dir) = cache_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);

    let Ok(monitor) = gio::File::for_path(&dir)
        .monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
    else {
        return;
    };

    monitor.connect_changed(move |_, file, _, event| {
        use gio::FileMonitorEvent::{ChangesDoneHint, MovedIn};
        if !matches!(event, ChangesDoneHint | MovedIn) {
            return;
        }
        let is_goto = file
            .basename()
            .map(|n| n.to_string_lossy() == "goto")
            .unwrap_or(false);
        if is_goto {
            handle_goto(&dir);
        }
    });

    GOTO_MONITOR.with(|cell| {
        let _ = cell.set(monitor);
    });
}

fn handle_goto(dir: &Path) {
    let goto = dir.join("goto");
    let Ok(text) = std::fs::read_to_string(&goto) else {
        return;
    };
    let target = text.trim();
    if target.is_empty() {
        return;
    }

    // Several windows may be open; only the one that navigated last answers.
    let owner = std::fs::read_to_string(dir.join("pid"))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok());
    if owner != Some(std::process::id()) {
        return;
    }

    // Consume the request so it cannot be replayed.
    let _ = std::fs::write(&goto, "");

    let path = PathBuf::from(target);
    if !path.is_dir() {
        return;
    }
    if let Some(sender) = crate::model::SENDER.get() {
        let _ = sender.send(crate::model::AppMsg::Navigate(path));
    }
    if let Some(app) =
        gio::Application::default().and_then(|a| a.downcast::<gtk::Application>().ok())
    {
        if let Some(window) = app.active_window() {
            window.present();
        }
    }
}
