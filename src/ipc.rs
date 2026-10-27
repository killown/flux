//! `--cwd` IPC: a Unix socket that lets the CLI ask a running window to
//! navigate to a directory.
//!
//! * Socket path: `~/.cache/flux/ipc.sock`.
//! * Server: background thread accepts connections, validates each request,
//!   pushes the canonicalized path into a bounded `std::sync::mpsc` channel
//!   drained by the GTK loop. Never blocks the UI thread.
//! * Client: connects, verifies the server's UID via `SO_PEERCRED`, writes
//!   `v1 cwd:<PATH>\n`, waits for `ok\n`/`err\n`.
//!
//! Security: socket dir `0o700`, socket `0o600`, client-side peer UID check,
//! server-side `canonicalize` + `is_dir`, stale-socket recovery, bounded
//! channel, strict `v1 cwd:`-only protocol.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Mutex, Once, OnceLock};
use std::thread;
use std::time::Duration;

const CMD_PREFIX: &str = "v1 cwd:";
const MAX_REQUEST_BYTES: u64 = 4096 + 64;
const SERVER_READ_TIMEOUT: Duration = Duration::from_secs(2);
const CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(2);

/// Returned by the client so `cli/mod.rs` can map outcomes to exit codes
/// and fallbacks without inspecting stringly-typed errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientOutcome {
    Ok,
    NotRunning,
    Rejected,
}

/// Result of trying to take ownership of the socket.
enum BindOutcome {
    /// We now own the listener.
    Bound(UnixListener),
    /// Another Flux instance is already listening. Not an error.
    AlreadyOwned,
    /// The socket file exists but we cannot connect to it and cannot remove
    /// it, something is wrong (usually permissions).
    Failed(std::io::Error),
}

fn socket_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("flux").join("ipc.sock"))
}

fn socket_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("flux"))
}

fn current_uid() -> u32 {
    unsafe { libc::geteuid() }
}

/// Reads `SO_PEERCRED` on a Unix socket fd and returns the peer's UID, or
/// `None` if the getsockopt call fails. Uses the stable `libc` binding rather
/// than `std::os::unix::net::UnixStream::peer_cred`, which is still nightly.
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let mut ucred = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            ucred.as_mut_ptr() as *mut libc::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return None;
    }
    Some(unsafe { ucred.assume_init() }.uid)
}

// ─── Server ──────────────────────────────────────────────────────────────────

static SERVER_STARTED: OnceLock<Mutex<()>> = OnceLock::new();
static SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Starts the `--cwd` server and returns the receiving end of the bounded
/// channel. The caller (the UI loop) must drain it and navigate.
///
/// Returns `None` (silently) when another Flux instance already owns the
/// socket. Any real failure to bind is logged.
pub fn start_server() -> Option<Receiver<PathBuf>> {
    let dir = socket_dir()?;
    let path = socket_path()?;

    let _guard = SERVER_STARTED
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));

    let listener = match try_bind(&path) {
        BindOutcome::Bound(l) => l,
        BindOutcome::AlreadyOwned => return None,
        BindOutcome::Failed(e) => {
            eprintln!("[ipc] cwd listener unavailable at {}: {e}", path.display());
            return None;
        }
    };
    if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)) {
        eprintln!("[ipc] chmod 0o600 failed on {}: {e}", path.display());
    }

    let _ = SOCKET_PATH.set(path);
    install_cleanup();

    let (tx, rx) = mpsc::sync_channel::<PathBuf>(16);

    let _ = thread::Builder::new()
        .name("flux-ipc-cwd".into())
        .spawn(move || accept_loop(listener, tx));

    Some(rx)
}

/// Removes the socket on process exit so the next Flux launch binds cleanly
/// instead of racing through the stale-socket recovery path.
fn install_cleanup() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        libc::atexit(cleanup_socket);
    });
}

extern "C" fn cleanup_socket() {
    if let Some(p) = SOCKET_PATH.get() {
        let _ = std::fs::remove_file(p);
    }
}

/// Binds the listener, recovering from a stale socket left by a crash, and
/// reporting "another instance owns it" separately from a genuine failure.
fn try_bind(path: &Path) -> BindOutcome {
    match UnixListener::bind(path) {
        Ok(l) => BindOutcome::Bound(l),
        Err(e) if e.kind() == ErrorKind::AddrInUse => probe_existing(path),
        Err(e) => BindOutcome::Failed(e),
    }
}

/// Decides what to do when a socket file already exists.
fn probe_existing(path: &Path) -> BindOutcome {
    match UnixStream::connect(path) {
        // A live server accepted our probe, do not clobber it.
        Ok(_) => BindOutcome::AlreadyOwned,
        // Socket is dead (crash, SIGKILL). Remove and rebind exactly once.
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
            if let Err(rm) = std::fs::remove_file(path) {
                if rm.kind() != ErrorKind::NotFound {
                    return BindOutcome::Failed(rm);
                }
            }
            match UnixListener::bind(path) {
                Ok(l) => BindOutcome::Bound(l),
                Err(e) => BindOutcome::Failed(e),
            }
        }
        // Permission denied usually means another user owns the socket,
        // report it instead of silently falling back.
        Err(e) => BindOutcome::Failed(e),
    }
}

fn accept_loop(listener: UnixListener, tx: SyncSender<PathBuf>) {
    for stream in listener.incoming() {
        match stream {
            Ok(s) => handle_connection(s, &tx),
            Err(e) => eprintln!("[ipc] cwd accept: {e}"),
        }
    }
}

fn handle_connection(mut stream: UnixStream, tx: &SyncSender<PathBuf>) {
    let _ = stream.set_read_timeout(Some(SERVER_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(SERVER_READ_TIMEOUT));

    match peer_uid(&stream) {
        Some(uid) if uid == current_uid() => {}
        Some(uid) => {
            eprintln!("[ipc] rejecting peer uid {uid}");
            return;
        }
        None => {
            eprintln!("[ipc] peer credentials unavailable");
            return;
        }
    }

    let mut line = Vec::new();
    let mut reader = BufReader::new((&stream).take(MAX_REQUEST_BYTES));
    if reader.read_until(b'\n', &mut line).is_err() {
        return;
    }
    if line.pop() != Some(b'\n') {
        let _ = stream.write_all(b"err\n");
        return;
    }

    let Some(rest) = line.strip_prefix(CMD_PREFIX.as_bytes()) else {
        let _ = stream.write_all(b"err\n");
        return;
    };

    let Ok(canon) = std::fs::canonicalize(OsStr::from_bytes(rest)) else {
        let _ = stream.write_all(b"err\n");
        return;
    };
    if !canon.is_dir() {
        let _ = stream.write_all(b"err\n");
        return;
    }

    if tx.send(canon).is_err() {
        let _ = stream.write_all(b"err\n");
        return;
    }

    let _ = stream.write_all(b"ok\n");
}

// ─── Client ──────────────────────────────────────────────────────────────────

/// Sends `v1 cwd:<path>` to the running window. `path` must be canonical.
pub fn send_cwd(path: &Path) -> ClientOutcome {
    let Some(sock) = socket_path() else {
        return ClientOutcome::NotRunning;
    };
    let Ok(mut stream) = UnixStream::connect(&sock) else {
        return ClientOutcome::NotRunning;
    };

    match peer_uid(&stream) {
        Some(uid) if uid == current_uid() => {}
        _ => return ClientOutcome::Rejected,
    }

    let _ = stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_READ_TIMEOUT));

    let bytes = path.as_os_str().as_bytes();
    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return ClientOutcome::Rejected;
    }
    let mut msg = Vec::with_capacity(CMD_PREFIX.len() + bytes.len() + 1);
    msg.extend_from_slice(CMD_PREFIX.as_bytes());
    msg.extend_from_slice(bytes);
    msg.push(b'\n');
    if msg.len() as u64 > MAX_REQUEST_BYTES {
        return ClientOutcome::Rejected;
    }
    if stream.write_all(&msg).is_err() {
        return ClientOutcome::NotRunning;
    }

    let mut answer = String::new();
    if BufReader::new(&stream).read_line(&mut answer).is_err() {
        return ClientOutcome::Rejected;
    }
    match answer.trim() {
        "ok" => ClientOutcome::Ok,
        _ => ClientOutcome::Rejected,
    }
}
