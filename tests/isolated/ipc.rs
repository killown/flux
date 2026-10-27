//! `--cwd` IPC: round trip, rejection paths and stale-socket recovery.

use flux::ipc::{send_cwd, start_server, ClientOutcome};
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

// The socket path comes from `XDG_CACHE_HOME`, so tests take turns owning it
// through the binary-wide `ENV_LOCK`.
use super::ENV_LOCK;

/// Points the cache dir at a fresh sandbox and returns it with the lock guard.
fn sandbox() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_CACHE_HOME", dir.path());
    (dir, guard)
}

fn sock(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("flux").join("ipc.sock")
}

/// Sends one raw line and returns the server's one-line answer.
fn raw_request(dir: &tempfile::TempDir, line: &str) -> String {
    let mut stream = UnixStream::connect(sock(dir)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream.write_all(line.as_bytes()).unwrap();
    let mut answer = String::new();
    BufReader::new(&stream).read_line(&mut answer).unwrap();
    answer
}

fn raw_bytes_request(dir: &tempfile::TempDir, bytes: &[u8]) -> String {
    let mut stream = UnixStream::connect(sock(dir)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let _ = stream.write_all(bytes);
    let _ = stream.shutdown(Shutdown::Write);
    let mut answer = String::new();
    let _ = BufReader::new(&stream).read_line(&mut answer);
    answer
}

#[test]
fn client_reports_not_running_without_a_server() {
    let (dir, _guard) = sandbox();
    let target = tempfile::tempdir().unwrap();
    assert_eq!(send_cwd(target.path()), ClientOutcome::NotRunning);
    drop(dir);
}

#[test]
fn directory_request_reaches_the_gtk_side_channel() {
    let (_dir, _guard) = sandbox();
    let rx = start_server().expect("first instance owns the socket");
    let target = tempfile::tempdir().unwrap();
    let canonical = target.path().canonicalize().unwrap();

    assert_eq!(send_cwd(&canonical), ClientOutcome::Ok);
    let got = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(got, canonical);
}

#[test]
fn second_instance_does_not_steal_the_socket() {
    let (_dir, _guard) = sandbox();
    let _rx = start_server().expect("first instance owns the socket");
    assert!(start_server().is_none(), "live socket must stay owned");

    let target = tempfile::tempdir().unwrap();
    assert_eq!(
        send_cwd(&target.path().canonicalize().unwrap()),
        ClientOutcome::Ok
    );
}

#[test]
fn stale_socket_left_by_a_crash_is_recovered() {
    let (dir, _guard) = sandbox();
    std::fs::create_dir_all(dir.path().join("flux")).unwrap();
    // Dropping a std listener leaves the socket file behind, like a SIGKILL.
    drop(UnixListener::bind(sock(&dir)).unwrap());
    assert!(sock(&dir).exists());

    assert!(start_server().is_some(), "stale socket must be rebound");
}

#[test]
fn files_and_missing_paths_are_rejected_and_never_forwarded() {
    let (dir, _guard) = sandbox();
    let rx = start_server().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("plain.txt");
    std::fs::write(&file, "x").unwrap();

    assert_eq!(send_cwd(&file), ClientOutcome::Rejected);
    assert_eq!(raw_request(&dir, "v1 cwd:/definitely/not/here\n"), "err\n");
    assert!(rx.try_recv().is_err());
}

#[test]
fn unknown_protocol_lines_are_rejected() {
    let (dir, _guard) = sandbox();
    let rx = start_server().unwrap();

    assert_eq!(raw_request(&dir, "garbage\n"), "err\n");
    assert_eq!(raw_request(&dir, "v2 cwd:/tmp\n"), "err\n");
    assert_eq!(raw_request(&dir, "cwd:/tmp\n"), "err\n");
    assert!(rx.try_recv().is_err());
}

#[test]
fn client_refuses_paths_containing_newlines() {
    let (_dir, _guard) = sandbox();
    let rx = start_server().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let evil = scratch.path().join("a\nb");
    std::fs::create_dir(&evil).unwrap();

    assert_eq!(
        send_cwd(&evil.canonicalize().unwrap()),
        ClientOutcome::Rejected
    );
    assert!(rx.try_recv().is_err());
}

#[test]
fn non_utf8_directory_is_navigated_to_byte_for_byte() {
    let (_dir, _guard) = sandbox();
    let rx = start_server().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let weird = scratch.path().join(OsStr::from_bytes(b"caf\xe9"));
    std::fs::create_dir(&weird).unwrap();
    let canonical = weird.canonicalize().unwrap();

    assert_eq!(send_cwd(&canonical), ClientOutcome::Ok);
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), canonical);
}

#[test]
fn request_without_newline_is_rejected_and_never_forwarded() {
    let (dir, _guard) = sandbox();
    let rx = start_server().unwrap();

    assert_eq!(raw_bytes_request(&dir, b"v1 cwd:/tmp"), "err\n");
    assert!(rx.try_recv().is_err());
}

#[test]
fn oversized_request_is_not_truncated_into_a_valid_path() {
    let (dir, _guard) = sandbox();
    let rx = start_server().unwrap();

    let mut request = b"v1 cwd:/tmp".to_vec();
    request.resize(8192, b'/');
    request.push(b'\n');
    let _ = raw_bytes_request(&dir, &request);

    assert!(rx.try_recv().is_err());
}
