//! Serves the current file selection to shells on demand.
//!
//! Every window listens on `$XDG_RUNTIME_DIR/flux/selection-<pid>.sock`. A
//! connection makes the window resolve its selection on the UI thread and
//! reply with the raw paths, each terminated by NUL, so the answer always
//! matches the grid. `flux-fm --selection` is the client behind
//! `flux_selection` in fish.

use crate::model::AppMsg;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// How long the UI thread gets to answer a selection query.
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("flux"))
}

/// Directory holding the sockets: `$XDG_RUNTIME_DIR/flux` (private to the
/// user and cleared on logout), falling back to the cache directory.
fn socket_dir() -> Option<PathBuf> {
    dirs::runtime_dir()
        .map(|d| d.join("flux"))
        .or_else(cache_dir)
}

fn socket_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("selection-{pid}.sock"))
}

fn process_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

/// Removes sockets left behind by windows that no longer exist.
fn remove_stale_sockets(dir: &Path) {
    crate::hit!("remove_stale_sockets");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let pid = name
            .to_string_lossy()
            .strip_prefix("selection-")
            .and_then(|rest| rest.strip_suffix(".sock"))
            .and_then(|pid| pid.parse::<u32>().ok());
        if pid.is_some_and(|pid| !process_alive(pid)) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Answers one connection with the paths currently selected in the active tab.
///
/// The selection lives in GTK objects that only the main thread may touch, so
/// the request is forwarded to the update loop and awaited with a timeout.
fn serve(mut stream: UnixStream) {
    crate::hit!("serve");
    let Some(sender) = crate::model::SENDER.get() else {
        return;
    };
    let (reply, paths) = mpsc::channel();
    if sender.send(AppMsg::QuerySelection { reply }).is_err() {
        return;
    }
    let Ok(paths) = paths.recv_timeout(REPLY_TIMEOUT) else {
        return;
    };

    let mut data = Vec::new();
    for path in paths {
        data.extend_from_slice(path.as_os_str().as_bytes());
        data.push(0);
    }
    let _ = stream.write_all(&data);
}

/// Starts the per-window selection socket. Errors are ignored: the socket is
/// a convenience and must never disturb the UI.
#[allow(dead_code)]
pub(crate) fn setup_selection_server() {
    crate::hit!("setup_selection_server");
    let Some(dir) = socket_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    // Closes the window between `bind` and the socket's own chmod below.
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    remove_stale_sockets(&dir);

    let path = socket_path(&dir, std::process::id());
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return;
    };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));

    let _ = std::thread::Builder::new()
        .name("flux-selection".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream);
            }
        });
}

/// Prints the selection of a running window to stdout and returns the process
/// exit status.
///
/// Paths are newline-terminated, or NUL-terminated when `null` is set. A path
/// containing a newline cannot be represented in the first form and is
/// skipped with a warning, so a shell can never split it into two arguments.
///
/// Targets the window named by `$FLUX_PID` (exported to Flux's embedded
/// terminal) while it is alive, otherwise the window that navigated last.
#[allow(dead_code)]
pub(crate) fn print_selection(null: bool) -> i32 {
    crate::hit!("print_selection");
    let (Some(cache), Some(dir)) = (cache_dir(), socket_dir()) else {
        return 1;
    };

    let pid = std::env::var("FLUX_PID")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|&pid| process_alive(pid))
        .or_else(|| {
            std::fs::read_to_string(cache.join("pid"))
                .ok()
                .and_then(|s| s.trim().parse().ok())
        });
    let Some(pid) = pid else {
        eprintln!("flux: no running window");
        return 1;
    };

    let Ok(mut stream) = UnixStream::connect(socket_path(&dir, pid)) else {
        eprintln!("flux: window {pid} is not answering");
        return 1;
    };
    let _ = stream.set_read_timeout(Some(REPLY_TIMEOUT + Duration::from_secs(1)));

    let mut data = Vec::new();
    if stream.read_to_end(&mut data).is_err() {
        return 1;
    }
    let mut out = Vec::with_capacity(data.len());
    for path in data.split(|&b| b == 0).filter(|p| !p.is_empty()) {
        if null {
            out.extend_from_slice(path);
            out.push(0);
        } else if path.contains(&b'\n') {
            eprintln!("flux: skipped a path containing a newline, use --selection -0");
        } else {
            out.extend_from_slice(path);
            out.push(b'\n');
        }
    }
    match std::io::stdout().lock().write_all(&out) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
