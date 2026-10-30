//! Shows, hides or toggles a running window over `$XDG_RUNTIME_DIR/flux/toggle-<pid>.sock`.

use crate::domain::model::{AppMsg, WindowMsg, ToggleVerb, WindowVisibility};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Once};
use std::time::Duration;

/// How long the UI thread gets to apply a verb.
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// Longest request line the server reads.
const MAX_REQUEST_BYTES: u64 = 64;

/// Directory holding the sockets: `$XDG_RUNTIME_DIR/flux`, falling back to the cache directory.
fn socket_dir() -> Option<PathBuf> {
    dirs::runtime_dir()
        .map(|d| d.join("flux"))
        .or_else(|| dirs::cache_dir().map(|d| d.join("flux")))
}

fn socket_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("toggle-{pid}.sock"))
}

/// Extracts the PID from a `toggle-<pid>.sock` file name.
fn socket_pid(name: &str) -> Option<u32> {
    name.strip_prefix("toggle-")?
        .strip_suffix(".sock")?
        .parse()
        .ok()
}

fn process_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

/// Removes sockets left behind by windows that no longer exist.
fn remove_stale_sockets(dir: &Path) {
    crate::hit!("window_control::remove_stale_sockets");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let pid = socket_pid(&entry.file_name().to_string_lossy());
        if pid.is_some_and(|pid| !process_alive(pid)) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn verb_name(verb: ToggleVerb) -> &'static str {
    match verb {
        ToggleVerb::Toggle => "toggle",
        ToggleVerb::Show => "show",
        ToggleVerb::Hide => "hide",
    }
}

fn parse_verb(line: &str) -> Option<ToggleVerb> {
    match line.trim() {
        "toggle" => Some(ToggleVerb::Toggle),
        "show" => Some(ToggleVerb::Show),
        "hide" => Some(ToggleVerb::Hide),
        _ => None,
    }
}

fn visibility_name(state: WindowVisibility) -> &'static str {
    match state {
        WindowVisibility::Visible => "visible",
        WindowVisibility::Hidden => "hidden",
    }
}

/// Answers one connection: reads a verb, has the UI thread apply it, writes the resulting state.
fn serve(mut stream: UnixStream) {
    crate::hit!("window_control::serve");
    let _ = stream.set_read_timeout(Some(REPLY_TIMEOUT));
    let mut line = String::new();
    if BufReader::new((&stream).take(MAX_REQUEST_BYTES))
        .read_line(&mut line)
        .is_err()
    {
        return;
    }
    let Some(verb) = parse_verb(&line) else {
        return;
    };
    let Some(sender) = crate::domain::model::SENDER.get() else {
        return;
    };
    let (reply, state) = mpsc::channel();
    if sender
        .send(AppMsg::Window(WindowMsg::ToggleWindowVisibility { verb, reply }))
        .is_err()
    {
        return;
    }
    let Ok(state) = state.recv_timeout(REPLY_TIMEOUT) else {
        return;
    };
    let _ = writeln!(stream, "{}", visibility_name(state));
}

/// Starts the per-window toggle socket once per process, ignoring errors.
pub fn setup_toggle_server() {
    crate::hit!("window_control::setup_toggle_server");
    static STARTED: Once = Once::new();
    STARTED.call_once(start_server);
}

fn start_server() {
    crate::hit!("start_server");
    let Some(dir) = socket_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    remove_stale_sockets(&dir);

    let path = socket_path(&dir, std::process::id());
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return;
    };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));

    let _ = std::thread::Builder::new()
        .name("flux-toggle".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream);
            }
        });
}

/// Sends `verb` to the window behind `pid` and reports whether it answered with a state.
fn request(dir: &Path, pid: u32, verb: ToggleVerb) -> bool {
    crate::hit!("request");
    let Ok(mut stream) = UnixStream::connect(socket_path(dir, pid)) else {
        return false;
    };
    let timeout = Some(REPLY_TIMEOUT + Duration::from_secs(1));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    if writeln!(stream, "{}", verb_name(verb)).is_err() {
        return false;
    }
    let mut answer = String::new();
    BufReader::new(&stream).read_line(&mut answer).is_ok()
        && matches!(answer.trim(), "visible" | "hidden")
}

/// Sends `verb` to running windows, highest PID first, and reports whether one answered.
fn send_to_running(verb: ToggleVerb) -> bool {
    crate::hit!("send_to_running");
    let Some(dir) = socket_dir() else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return false;
    };
    let mut pids: Vec<u32> = entries
        .flatten()
        .filter_map(|e| socket_pid(&e.file_name().to_string_lossy()))
        .filter(|&pid| process_alive(pid))
        .collect();
    pids.sort_unstable_by(|a, b| b.cmp(a));
    pids.into_iter().any(|pid| request(&dir, pid, verb))
}

/// Toggles the running window, returns `0` when one answered and `1` when none is running.
pub fn toggle_running() -> i32 {
    crate::hit!("window_control::toggle_running");
    i32::from(!send_to_running(ToggleVerb::Toggle))
}

/// Shows the running window, returns `0` when one answered and `1` when none is running.
pub fn show_running() -> i32 {
    crate::hit!("window_control::show_running");
    i32::from(!send_to_running(ToggleVerb::Show))
}

/// Hides the running window, returns `0` also when none is running, as there is nothing to hide.
pub fn hide_running() -> i32 {
    crate::hit!("window_control::hide_running");
    let _ = send_to_running(ToggleVerb::Hide);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_pid_round_trips() {
        let path = socket_path(Path::new("/run/user/1000/flux"), 4242);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(socket_pid(&name), Some(4242));
        assert_eq!(socket_pid("selection-4242.sock"), None);
        assert_eq!(socket_pid("toggle-abc.sock"), None);
    }

    #[test]
    fn verbs_round_trip() {
        for verb in [ToggleVerb::Toggle, ToggleVerb::Show, ToggleVerb::Hide] {
            assert_eq!(parse_verb(&format!("{}\n", verb_name(verb))), Some(verb));
        }
        assert_eq!(parse_verb("quit\n"), None);
        assert_eq!(parse_verb(""), None);
    }
}
