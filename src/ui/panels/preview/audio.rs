//! Audio preview through a detached `mpv` child process, driven over its JSON IPC
//! socket.

use super::handle::PreviewPanelHandle;
use super::media::PREVIEW_MUTED;
use super::registry;
use crate::i18n::tr;
use adw::prelude::*;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

pub(super) static AUDIO_MPV_PID: AtomicU64 = AtomicU64::new(0);
/// Monotonic counter that gives every mpv spawn its own sequence number.
static AUDIO_SEQ: AtomicU64 = AtomicU64::new(0);
/// Sequence number of the mpv instance the panel is bound to, 0 when none.
static AUDIO_ACTIVE: AtomicU64 = AtomicU64::new(0);
/// Writer half of the active mpv IPC socket.
static AUDIO_IPC: Mutex<Option<UnixStream>> = Mutex::new(None);
/// Monotonic time (µs) until which `time-pos` updates must not move the seek bar.
pub(super) static AUDIO_SEEK_HOLD: AtomicI64 = AtomicI64::new(0);

/// Scalar value of an mpv `property-change` event.
#[derive(Clone, Copy, PartialEq)]
enum MpvValue {
    Num(f64),
    Bool(bool),
    Null,
}

/// Sends one JSON command line to the active mpv instance, ignoring failures.
pub(super) fn ipc_send(cmd: &str) {
    let mut guard = AUDIO_IPC.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(stream) = guard.as_mut() {
        let _ = stream.write_all(cmd.as_bytes());
        let _ = stream.write_all(b"\n");
    }
}

/// Returns the raw scalar or unquoted string following `"key":` in a flat JSON line.
fn json_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\":");
    let start = line.find(&pat)? + pat.len();
    let rest = line[start..].trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        let end = quoted.find('"')?;
        Some(&quoted[..end])
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim())
    }
}

/// Parses an mpv `property-change` event line into `(property, value)`.
fn parse_property_event(line: &str) -> Option<(String, MpvValue)> {
    if json_field(line, "event")? != "property-change" {
        return None;
    }
    let name = json_field(line, "name")?.to_string();
    let value = match json_field(line, "data") {
        Some("true") => MpvValue::Bool(true),
        Some("false") => MpvValue::Bool(false),
        Some(raw) => raw.parse().map(MpvValue::Num).unwrap_or(MpvValue::Null),
        None => MpvValue::Null,
    };
    Some((name, value))
}

/// Formats seconds as `m:ss`, or `h:mm:ss` from one hour up.
fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Connects to mpv's IPC socket, observes playback properties and forwards them to the panel.
fn spawn_ipc_thread(sock: PathBuf, seq: u64) {
    crate::hit!("spawn_ipc_thread");
    std::thread::spawn(move || {
        let mut connected = None;
        for _ in 0..80 {
            if AUDIO_ACTIVE.load(Ordering::SeqCst) != seq {
                return;
            }
            if let Ok(stream) = UnixStream::connect(&sock) {
                connected = Some(stream);
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let Some(stream) = connected else { return };
        let Ok(reader) = stream.try_clone() else {
            return;
        };
        let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
        if AUDIO_ACTIVE.load(Ordering::SeqCst) != seq {
            return;
        }
        *AUDIO_IPC.lock().unwrap_or_else(|e| e.into_inner()) = Some(stream);

        for (id, prop) in ["time-pos", "duration", "pause", "volume", "eof-reached"]
            .iter()
            .enumerate()
        {
            ipc_send(&format!(
                r#"{{"command":["observe_property",{},"{prop}"]}}"#,
                id + 1
            ));
        }
        glib::idle_add_once(move || {
            if AUDIO_ACTIVE.load(Ordering::SeqCst) == seq {
                registry::with(|h| h.audio_set_interactive(true));
            }
        });

        let mut last_pos = f64::NEG_INFINITY;
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if AUDIO_ACTIVE.load(Ordering::SeqCst) != seq {
                break;
            }
            let Some((name, value)) = parse_property_event(&line) else {
                continue;
            };
            if let (MpvValue::Num(pos), "time-pos") = (value, name.as_str()) {
                if (pos - last_pos).abs() < 0.25 {
                    continue;
                }
                last_pos = pos;
            }
            glib::idle_add_once(move || {
                if AUDIO_ACTIVE.load(Ordering::SeqCst) == seq {
                    registry::with(|h| h.audio_on_property(&name, value));
                }
            });
        }
    });
}

pub(super) fn stop_audio_mpv() {
    crate::hit!("stop_audio_mpv");
    AUDIO_ACTIVE.store(0, Ordering::SeqCst);
    *AUDIO_IPC.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let pid = AUDIO_MPV_PID.swap(0, Ordering::SeqCst);
    if pid == 0 {
        return;
    }
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
        libc::kill(pid as i32, libc::SIGTERM);
    }
}

fn spawn_audio_mpv(path: &Path) -> Result<u64, String> {
    crate::hit!("spawn_audio_mpv");
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let mpv = glib::find_program_in_path("mpv").ok_or_else(|| {
        "mpv not installed - audio preview requires the `mpv` binary on PATH".to_string()
    })?;

    let seq = AUDIO_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    let sock = glib::user_runtime_dir().join(format!("flux-mpv-{}-{seq}.sock", std::process::id()));

    let mut cmd = Command::new(mpv);
    cmd.args([
        "--no-video",
        "--no-terminal",
        "--audio-device=auto",
        "--ao=pipewire,pulse,alsa",
        "--msg-level=all=warn",
        "--keep-open=yes",
    ])
    .arg(if PREVIEW_MUTED.load(Ordering::SeqCst) {
        "--mute=yes"
    } else {
        "--no-mute"
    })
    .arg(format!("--input-ipc-server={}", sock.display()))
    .arg("--")
    .arg(path)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());

    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong);
            Ok(())
        });
    }

    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id() as u64;
            AUDIO_ACTIVE.store(seq, Ordering::SeqCst);
            let sock_wait = sock.clone();
            std::thread::spawn(move || {
                let status = child.wait();
                let _ = std::fs::remove_file(&sock_wait);
                if AUDIO_MPV_PID
                    .compare_exchange(pid, 0, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let failed = match status {
                        Ok(s) if s.success() => None,
                        Ok(s) => Some(s.code().unwrap_or(-1)),
                        Err(_) => Some(-1),
                    };
                    glib::idle_add_once(move || {
                        registry::with(|h| {
                            h.audio_reset_controls();
                            h.audio_status.set_label(&match failed {
                                None => tr("Finished."),
                                Some(code) => {
                                    format!("{} ({code})", tr("mpv could not play this file"))
                                }
                            });
                        });
                    });
                }
            });
            spawn_ipc_thread(sock, seq);
            Ok(pid)
        }
        Err(e) => Err(format!("Failed to spawn mpv: {e}")),
    }
}

impl PreviewPanelHandle {
    /// Enables or disables every audio control except the play button.
    fn audio_set_interactive(&self, on: bool) {
        self.audio_seek.set_sensitive(on);
        self.audio_vol.set_sensitive(on);
        self.audio_back.set_sensitive(on);
        self.audio_fwd.set_sensitive(on);
    }

    /// Returns the audio controls to their idle state.
    pub(super) fn audio_reset_controls(&self) {
        self.audio_set_interactive(false);
        self.audio_seek.set_range(0.0, 1.0);
        self.audio_seek.set_value(0.0);
        self.audio_time.set_label("0:00");
        self.audio_dur.set_label("0:00");
        self.audio_btn.set_label(&tr("Play"));
    }

    /// Applies one observed mpv property to the controls.
    fn audio_on_property(&self, name: &str, value: MpvValue) {
        match (name, value) {
            ("time-pos", MpvValue::Num(t)) => {
                if glib::monotonic_time() >= AUDIO_SEEK_HOLD.load(Ordering::SeqCst) {
                    self.audio_seek.set_value(t);
                }
                self.audio_time.set_label(&fmt_time(t));
            }
            ("duration", MpvValue::Num(d)) if d > 0.0 => {
                self.audio_seek.set_range(0.0, d);
                self.audio_dur.set_label(&fmt_time(d));
            }
            ("pause", MpvValue::Bool(paused)) => {
                self.audio_btn
                    .set_label(&if paused { tr("Play") } else { tr("Pause") });
            }
            ("volume", MpvValue::Num(v)) => self.audio_vol.set_value(v),
            // `--keep-open=yes` parks mpv at the end instead of exiting.
            ("eof-reached", MpvValue::Bool(true)) => self.playlist_advance(),
            _ => {}
        }
    }

    /// Play/pause toggle that restarts playback when mpv is gone or sitting at the end.
    pub(super) fn audio_toggle(&self) {
        if AUDIO_MPV_PID.load(Ordering::SeqCst) == 0 {
            self.audio_status.set_label("");
            self.start_audio();
            return;
        }
        let upper = self.audio_seek.adjustment().upper();
        if upper > 1.0 && self.audio_seek.value() >= upper - 0.25 {
            ipc_send(r#"{"command":["seek",0,"absolute"]}"#);
            ipc_send(r#"{"command":["set_property","pause",false]}"#);
        } else {
            ipc_send(r#"{"command":["cycle","pause"]}"#);
        }
    }

    pub(super) fn start_audio(&self) {
        crate::hit!("start_audio");
        if AUDIO_MPV_PID.load(Ordering::SeqCst) != 0 {
            return;
        }
        let Some(path) = self.state.path.borrow().clone() else {
            return;
        };
        match spawn_audio_mpv(&path) {
            Ok(pid) => {
                AUDIO_MPV_PID.store(pid, Ordering::SeqCst);
                self.audio_btn.set_label(&tr("Pause"));
                self.audio_status.set_label("");
            }
            Err(msg) => {
                eprintln!("[flux][preview] audio: {msg}");
                self.audio_status.set_label(&tr(
                    "Audio preview requires mpv. Install it with your package manager.",
                ));
                self.audio_btn.set_sensitive(false);
            }
        }
    }
}
