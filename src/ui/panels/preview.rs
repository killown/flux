//! Peek preview side panel. Toggled by `Ctrl+Space` from the file grid,
//! follows selection while open.
//!
//! Every file gets a type-specific viewer: crisp PDF pages, full-resolution
//! images, syntect-highlighted text with a line-number gutter, Markdown,
//! CSV/TSV tables, hex dumps for binaries, folder / archive listings, or
//! media tags. Audio playback is delegated to `mpv` as a detached child
//! process. Video playback uses `gtk::Video` (GStreamer). The two paths are
//! deliberately kept separate because GStreamer's `decodebin3` aborts the
//! process on mislabeled containers, while mpv routes through FFmpeg.

use super::header::panel_header;
use super::preview_folder_thumbs::FolderThumbs;
use super::preview_gutter::LineNumberGutter;
use super::preview_markdown::{MdStyle, StyledRange};
use super::preview_registry;
use super::preview_syntax::HighlightedRange;
use super::preview_table::{self, TableData};
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::FileItem;
use adw::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use relm4::AsyncComponentSender;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::ops::Range;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

// ─── Content model ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub mtime: i64,
    pub extra: Vec<(String, String)>,
}

/// One entry of the folder grid shown for a directory.
#[derive(Debug, Clone)]
pub struct FolderEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone)]
pub enum PreviewContent {
    Loading,
    Image {
        tex: gtk::gdk::Texture,
        info: FileInfo,
    },
    Pdf {
        pages: usize,
        info: FileInfo,
        render_path: PathBuf,
    },
    Text {
        head: String,
        truncated: bool,
        hex: bool,
        info: FileInfo,
        highlighted: Vec<HighlightedRange>,
        language: Option<String>,
    },
    Markdown {
        rendered: String,
        styles: Vec<StyledRange>,
        source: String,
        info: FileInfo,
    },
    Table {
        data: TableData,
        info: FileInfo,
    },
    Folder {
        entries: Vec<FolderEntry>,
        more: usize,
        info: FileInfo,
    },
    Media {
        path: PathBuf,
        video: bool,
        cover: Option<gtk::gdk::Texture>,
        info: FileInfo,
    },
    Metadata {
        info: FileInfo,
    },
    Error(String),
}

// ─── Tunables ─────────────────────────────────────────────────────────────────

const ZOOM_MIN: f64 = 0.25;
const ZOOM_MAX: f64 = 5.0;
const ZOOM_STEP: f64 = 1.25;
const MAX_PDF_PX: i32 = 4096;
const MAX_PDF_PX_H: i32 = 16384;
const PDF_GUTTER: i32 = 24;
const INFO_MAX_WIDTH: i32 = 720;
/// After a playlist auto-advance, non-media items stay skippable for this long (µs).
/// Bounded so a manual selection later (e.g. after the last track) is never skipped.
const PLAYLIST_SKIP_WINDOW_US: i64 = 3_000_000;
/// A video counts as finished once its position is this close to its duration (µs).
const PLAYLIST_END_SLACK_US: i64 = 300_000;
/// A backwards jump from this close to the end is a loop wrap-around, not a user seek (µs).
const PLAYLIST_WRAP_ZONE_US: i64 = 1_500_000;

#[cfg(target_endian = "little")]
const CAIRO_ARGB32: gtk::gdk::MemoryFormat = gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied;
#[cfg(target_endian = "big")]
const CAIRO_ARGB32: gtk::gdk::MemoryFormat = gtk::gdk::MemoryFormat::A8r8g8b8Premultiplied;

fn body_font_family() -> String {
    gtk::Settings::default()
        .and_then(|s| s.gtk_font_name())
        .and_then(|n| {
            pango::FontDescription::from_string(&n)
                .family()
                .map(|f| f.to_string())
        })
        .unwrap_or_else(|| "sans-serif".to_string())
}

// ─── Audio via mpv ───────────────────────────────────────────────────────────

static AUDIO_MPV_PID: AtomicU64 = AtomicU64::new(0);
/// Monotonic counter that gives every mpv spawn its own sequence number.
static AUDIO_SEQ: AtomicU64 = AtomicU64::new(0);
/// Sequence number of the mpv instance the panel is bound to, 0 when none.
static AUDIO_ACTIVE: AtomicU64 = AtomicU64::new(0);
/// Writer half of the active mpv IPC socket.
static AUDIO_IPC: Mutex<Option<UnixStream>> = Mutex::new(None);
/// Monotonic time (µs) until which `time-pos` updates must not move the seek bar.
static AUDIO_SEEK_HOLD: AtomicI64 = AtomicI64::new(0);
/// Mute toggle of the header button, applies to video and audio, reset when the panel closes.
static PREVIEW_MUTED: AtomicBool = AtomicBool::new(false);

/// Scalar value of an mpv `property-change` event.
#[derive(Clone, Copy, PartialEq)]
enum MpvValue {
    Num(f64),
    Bool(bool),
    Null,
}

/// Sends one JSON command line to the active mpv instance, ignoring failures.
fn ipc_send(cmd: &str) {
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
                preview_registry::with(|h| h.audio_set_interactive(true));
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
                    preview_registry::with(|h| h.audio_on_property(&name, value));
                }
            });
        }
    });
}

fn stop_audio_mpv() {
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
                        preview_registry::with(|h| {
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

// ─── View state ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    None,
    Pdf,
    Image,
    Text,
    Markdown,
    Table,
}

/// Style key for deduplicating syntax-highlight text tags:
/// (foreground RGB, bold, italic, underline).
type SyntaxStyleKey = ((u8, u8, u8), bool, bool, bool);

pub(crate) struct State {
    mode: Cell<Mode>,
    /// Bumped on every content change, async results with an older token
    /// are discarded.
    token: Cell<u64>,
    path: RefCell<Option<PathBuf>>,
    pages: Cell<usize>,
    page: Cell<usize>,
    zoom: Cell<f64>,
    suppress: Cell<bool>,
    rendered_w: Cell<i32>,
    resize_gen: Cell<u64>,
    tex_px: Cell<(i32, i32)>,
    last_image_view: Cell<(i32, i32)>,
    media: RefCell<Option<gtk::MediaFile>>,
    pdf_path: RefCell<Option<PathBuf>>,
    /// gtk4-rs does not expose an iterator over `TextTagTable`, track the
    /// installed names ourselves so they can be removed on rebuild.
    syntax_tags: RefCell<Vec<String>>,
    /// Cached rendered markdown so the source toggle can flip back without
    /// re-parsing.
    md_cache: RefCell<Option<(String, Vec<StyledRange>)>>,
    /// Guards `mime_filter_btn` against re-entrant `connect_toggled` while
    /// its active state is being pushed from `FluxApp`.
    mime_filter_suppress: Cell<bool>,
    /// Lazy image and video thumbnails of the folder grid.
    folder_thumbs: FolderThumbs,
    /// Current icon size of the folder grid cells (Ctrl+scroll changes it).
    folder_icon_size: Cell<i32>,
    /// Entries of the folder shown, kept so a zoom can refill the grid.
    folder_entries: RefCell<Vec<FolderEntry>>,
    /// Bumped whenever the folder grid is refilled or released, a chunked fill
    /// still in progress stops when it no longer matches.
    fill_gen: Cell<u64>,
    /// Playlist mode: when the playing video/audio ends, step to the next item.
    playlist: Cell<bool>,
    /// Monotonic time (µs) until which non-media items are skipped after an auto-advance.
    playlist_skip_until: Cell<i64>,
}

#[derive(Clone)]
pub struct PreviewPanelHandle {
    pub(crate) stack: gtk::Stack,
    pub(crate) nav_box: gtk::Box,
    pub(crate) page_spin: gtk::SpinButton,
    pub(crate) page_total: gtk::Label,
    pub(crate) prev_btn: gtk::Button,
    pub(crate) next_btn: gtk::Button,
    pub(crate) wrap_btn: gtk::ToggleButton,
    pub(crate) invert_btn: gtk::ToggleButton,
    pub(crate) md_source_btn: gtk::ToggleButton,
    pub(crate) wide_btn: gtk::ToggleButton,
    pub(crate) fullscreen_btn: gtk::Button,
    pub(crate) mute_btn: gtk::ToggleButton,
    pub(crate) header: gtk::Box,
    pub(crate) folder_grid: Rc<RefCell<TypedGridView<FileItem, gtk::MultiSelection>>>,
    pub(crate) mime_filter_btn: gtk::ToggleButton,
    pub(crate) playlist_btn: gtk::ToggleButton,
    pub(crate) install_font_btn: gtk::Button,
    pub(crate) zoom_btn: gtk::Button,
    pub(crate) banner: adw::Banner,
    pub(crate) image: gtk::Picture,
    pub(crate) image_scroller: gtk::ScrolledWindow,
    pub(crate) text_view: gtk::TextView,
    pub(crate) text_tag: gtk::TextTag,
    pub(crate) md_base_tag: gtk::TextTag,
    pub(crate) pdf_scroller: gtk::ScrolledWindow,
    pub(crate) pdf_picture: gtk::Picture,
    pub(crate) table_view: gtk::ColumnView,
    pub(crate) meta_list: gtk::ListBox,
    pub(crate) nopreview: adw::StatusPage,
    pub(crate) footer: gtk::Box,
    pub(crate) switcher: adw::ViewSwitcher,
    pub(crate) view_stack: adw::ViewStack,
    pub(crate) details_list: gtk::ListBox,
    pub(crate) media_video: gtk::Video,
    pub(crate) media_cover: gtk::Picture,
    pub(crate) audio_art_stack: gtk::Stack,
    pub(crate) audio_status: gtk::Label,
    pub(crate) audio_btn: gtk::Button,
    pub(crate) audio_back: gtk::Button,
    pub(crate) audio_fwd: gtk::Button,
    pub(crate) audio_seek: gtk::Scale,
    pub(crate) audio_vol: gtk::Scale,
    pub(crate) audio_time: gtk::Label,
    pub(crate) audio_dur: gtk::Label,
    pub(crate) state: Rc<State>,
}

impl PreviewPanelHandle {
    fn rebuild_syntax_tags(&self, ranges: &[HighlightedRange]) -> Vec<(String, Range<usize>)> {
        crate::hit!("rebuild_syntax_tags");
        let table = self.text_view.buffer().tag_table();
        for name in self.state.syntax_tags.borrow_mut().drain(..) {
            if let Some(tag) = table.lookup(&name) {
                table.remove(&tag);
            }
        }

        let mut by_style: HashMap<SyntaxStyleKey, String> = HashMap::new();
        let mut installed: Vec<String> = Vec::new();
        let mut out = Vec::with_capacity(ranges.len());
        for r in ranges {
            let key = (r.fg, r.bold, r.italic, r.underline);
            let name = by_style
                .entry(key)
                .or_insert_with(|| {
                    let name = format!("flux-syn-{}", installed.len());
                    let tag = gtk::TextTag::builder()
                        .name(&name)
                        .foreground(format!("#{:02x}{:02x}{:02x}", r.fg.0, r.fg.1, r.fg.2))
                        .weight(if r.bold { 700 } else { 400 })
                        .style(if r.italic {
                            gtk::pango::Style::Italic
                        } else {
                            gtk::pango::Style::Normal
                        })
                        .underline(if r.underline {
                            gtk::pango::Underline::Single
                        } else {
                            gtk::pango::Underline::None
                        })
                        .build();
                    table.add(&tag);
                    installed.push(name.clone());
                    name
                })
                .clone();
            out.push((name, r.start..r.end));
        }
        *self.state.syntax_tags.borrow_mut() = installed;
        out
    }

    /// Markdown styles share the same tag-tracking scheme as syntect.
    ///
    /// Non-code styles pin the family to the current GTK UI font so body
    /// text matches the rest of the application, code spans and blocks
    /// opt back into monospace.
    fn rebuild_markdown_tags(&self, styles: &[StyledRange]) -> Vec<(String, Range<usize>)> {
        crate::hit!("rebuild_markdown_tags");
        let table = self.text_view.buffer().tag_table();
        for name in self.state.syntax_tags.borrow_mut().drain(..) {
            if let Some(tag) = table.lookup(&name) {
                table.remove(&tag);
            }
        }

        let body_family = body_font_family();
        let colors = super::preview_syntax::tag_colors();

        let mut out = Vec::with_capacity(styles.len());
        let mut installed = Vec::with_capacity(styles.len());
        for (i, s) in styles.iter().enumerate() {
            let name = format!("flux-syn-{i}");
            let mut b = gtk::TextTag::builder().name(&name);
            b = match s.style {
                MdStyle::Heading1 => b.family(&body_family).scale(1.35).weight(700),
                MdStyle::Heading2 => b.family(&body_family).scale(1.22).weight(700),
                MdStyle::Heading3 => b.family(&body_family).scale(1.12).weight(700),
                MdStyle::Heading4 => b.family(&body_family).scale(1.05).weight(700),
                MdStyle::Heading5 => b.family(&body_family).scale(1.0).weight(700),
                MdStyle::Heading6 => b.family(&body_family).scale(0.95).weight(700),
                MdStyle::Bold => b.family(&body_family).weight(700),
                MdStyle::Italic => b.family(&body_family).style(gtk::pango::Style::Italic),
                MdStyle::Code => b
                    .family("monospace")
                    .background(colors.code_bg)
                    .foreground(colors.code_fg),
                MdStyle::CodeBlock => b.family("monospace").foreground(colors.code_block),
                MdStyle::Link => b
                    .family(&body_family)
                    .foreground(colors.link)
                    .underline(gtk::pango::Underline::Single),
                MdStyle::BlockQuote => b
                    .family(&body_family)
                    .foreground(colors.quote)
                    .style(gtk::pango::Style::Italic),
                MdStyle::ListBullet => b.family(&body_family).weight(600),
            };
            let tag = b.build();
            table.add(&tag);
            installed.push(name.clone());
            out.push((name, s.range.clone()));
        }
        self.state.syntax_tags.borrow_mut().clone_from(&installed);
        out
    }
}

// ─── Construction ─────────────────────────────────────────────────────────────

pub fn build_preview_panel(initial_width: i32, sender: AsyncComponentSender<FluxApp>) -> gtk::Box {
    crate::hit!("build_preview_panel");
    install_css();
    let spec = PanelSpec::preview().with_initial(initial_width);

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    // Header row. All controls are hoisted into `panel_header`'s `extra`
    // slot so the panel keeps a single HIG-style strip, matching Search /
    // Diff / Tag / Location / Connect.
    let nav_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .build();
    let prev_btn = flat_icon_button("go-previous-symbolic", &tr("Previous page"));
    let next_btn = flat_icon_button("go-next-symbolic", &tr("Next page"));
    let page_spin = gtk::SpinButton::with_range(1.0, 1.0, 1.0);
    page_spin.set_numeric(true);
    page_spin.set_width_chars(3);
    let page_total = gtk::Label::builder().css_classes(["dim-label"]).build();
    nav_box.append(&prev_btn);
    nav_box.append(&page_spin);
    nav_box.append(&page_total);
    nav_box.append(&next_btn);
    nav_box.set_visible(false);

    let wrap_btn = gtk::ToggleButton::builder()
        .icon_name("format-justify-fill-symbolic")
        .tooltip_text(tr("Wrap lines"))
        .css_classes(["flat"])
        .visible(false)
        .build();
    let invert_btn = gtk::ToggleButton::builder()
        .icon_name("weather-clear-night-symbolic")
        .tooltip_text(tr("Invert colors"))
        .css_classes(["flat"])
        .visible(false)
        .build();
    let md_source_btn = gtk::ToggleButton::builder()
        .icon_name("text-x-generic-symbolic")
        .tooltip_text(tr("Toggle Markdown source"))
        .css_classes(["flat"])
        .visible(false)
        .build();
    let install_font_btn =
        flat_icon_button("system-software-install-symbolic", &tr("Install font"));
    install_font_btn.set_visible(false);
    let zoom_out = flat_icon_button("zoom-out-symbolic", &tr("Zoom out"));
    let zoom_in = flat_icon_button("zoom-in-symbolic", &tr("Zoom in"));
    let zoom_btn = gtk::Button::builder()
        .label("100%")
        .tooltip_text(tr("Reset zoom"))
        .css_classes(["flat"])
        .build();

    let details_toggle = gtk::ToggleButton::builder()
        .icon_name("dialog-information-symbolic")
        .tooltip_text(tr("Show file details"))
        .css_classes(["flat"])
        .build();

    let wide_btn = gtk::ToggleButton::builder()
        .icon_name("view-dual-symbolic")
        .tooltip_text(tr("Always open preview wide (F11)"))
        .css_classes(["flat"])
        .active(crate::utils::load_config().ui.wide_preview)
        .build();

    let fullscreen_btn = flat_icon_button(
        &first_available_icon(&["view-fullscreen-symbolic", "zoom-fit-best-symbolic"]),
        &tr("Full screen"),
    );

    // Only shown while a video or audio file is previewed (see `apply`).
    let mute_btn = gtk::ToggleButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .tooltip_text(tr("Mute media (until the preview is closed)"))
        .css_classes(["flat"])
        .visible(false)
        .build();

    let mime_filter_btn = gtk::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(tr("Show only files of this type (Ctrl+M)"))
        .css_classes(["flat"])
        .build();

    // Only shown while a video or audio file is previewed (see `apply`).
    // Off shows "repeat" (media loops by default), on shows "play in sequence".
    let playlist_icon_off =
        first_available_icon(&["media-playlist-repeat-symbolic", "view-refresh-symbolic"]);
    let playlist_icon_on =
        first_available_icon(&["media-playlist-consecutive-symbolic", "view-list-symbolic"]);
    let playlist_btn = gtk::ToggleButton::builder()
        .icon_name(playlist_icon_off.as_str())
        .tooltip_text(tr("Playlist mode: play the next item when playback ends"))
        .css_classes(["flat"])
        .visible(false)
        .build();

    let spacer = gtk::Box::builder().hexpand(true).build();

    let banner = adw::Banner::builder().revealed(false).build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .hexpand(true)
        .build();

    let empty = adw::StatusPage::builder()
        .icon_name("eye-open-symbolic")
        .title(tr("No preview"))
        .description(tr("Select an item to preview."))
        .build();
    stack.add_named(&empty, Some("empty"));

    let loading = gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    stack.add_named(&loading, Some("loading"));

    let nopreview = adw::StatusPage::builder()
        .icon_name("text-x-generic-symbolic")
        .title(tr("No preview available"))
        .build();
    stack.add_named(&nopreview, Some("nopreview"));

    let media_video = gtk::Video::builder()
        .vexpand(true)
        .hexpand(true)
        .loop_(true)
        .autoplay(true)
        .build();
    // gtk::Video advertises the clip's intrinsic size as its natural size.
    // A non-propagating ScrolledWindow keeps that out of the panel's width
    // negotiation, otherwise the resize drag would follow the video.
    let media_video_frame = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .hexpand(true)
        .vexpand(true)
        .child(&media_video)
        .build();
    stack.add_named(&media_video_frame, Some("media-video"));

    let media_cover = gtk::Picture::builder()
        .can_shrink(true)
        .vexpand(true)
        .margin_top(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    let audio_icon = gtk::Image::builder()
        .icon_name("audio-x-generic-symbolic")
        .pixel_size(128)
        .vexpand(true)
        .css_classes(["dim-label"])
        .build();
    let audio_art_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();
    audio_art_stack.add_named(&media_cover, Some("cover"));
    audio_art_stack.add_named(&audio_icon, Some("icon"));

    let audio_status = gtk::Label::builder()
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dim-label"])
        .margin_start(16)
        .margin_end(16)
        .build();

    let audio_btn = gtk::Button::builder()
        .label(tr("Play"))
        .css_classes(["pill", "suggested-action"])
        .build();
    let audio_back = flat_icon_button("media-seek-backward-symbolic", &tr("Back 10 seconds"));
    let audio_fwd = flat_icon_button("media-seek-forward-symbolic", &tr("Forward 10 seconds"));
    let audio_time = gtk::Label::builder()
        .label("0:00")
        .css_classes(["dim-label", "numeric"])
        .build();
    let audio_dur = gtk::Label::builder()
        .label("0:00")
        .css_classes(["dim-label", "numeric"])
        .build();
    let audio_seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    audio_seek.set_draw_value(false);
    audio_seek.set_hexpand(true);
    let audio_vol = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    audio_vol.set_draw_value(false);
    audio_vol.set_width_request(96);
    audio_vol.set_tooltip_text(Some(&tr("Volume")));

    let audio_seek_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(16)
        .margin_end(16)
        .build();
    audio_seek_row.append(&audio_time);
    audio_seek_row.append(&audio_seek);
    audio_seek_row.append(&audio_dur);

    let audio_buttons_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .build();
    audio_buttons_row.append(&audio_back);
    audio_buttons_row.append(&audio_btn);
    audio_buttons_row.append(&audio_fwd);

    let audio_vol_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .margin_bottom(16)
        .build();
    audio_vol_row.append(&gtk::Image::from_icon_name("audio-volume-high-symbolic"));
    audio_vol_row.append(&audio_vol);

    let audio_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    audio_box.append(&audio_art_stack);
    audio_box.append(&audio_status);
    audio_box.append(&audio_seek_row);
    audio_box.append(&audio_buttons_row);
    audio_box.append(&audio_vol_row);
    stack.add_named(&audio_box, Some("media-audio"));

    let image = gtk::Picture::builder()
        .can_shrink(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    let image_scroller = gtk::ScrolledWindow::builder()
        .child(&image)
        .vexpand(true)
        .build();
    stack.add_named(&image_scroller, Some("image"));

    // Text and Markdown share this view, only the buffer and tag table
    // differ between the two arms of `apply`.
    let text_view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::None)
        .top_margin(16)
        .bottom_margin(16)
        .left_margin(16)
        .right_margin(16)
        .pixels_above_lines(3)
        .pixels_below_lines(3)
        .build();
    let text_tag = gtk::TextTag::builder().name("flux-zoom").scale(1.0).build();
    text_view.buffer().tag_table().add(&text_tag);

    // Applied to the whole Markdown buffer first, so unstyled runs between
    // styled spans do not fall back to whatever family the widget chain
    // happens to provide.
    let md_base_tag = gtk::TextTag::builder()
        .name("flux-md-base")
        .family(body_font_family())
        .build();
    text_view.buffer().tag_table().add(&md_base_tag);

    // The view itself is proportional, plain-text previews apply this tag
    // to the whole buffer to opt into monospace.
    let mono_tag = gtk::TextTag::builder()
        .name("flux-mono")
        .family("monospace")
        .build();
    text_view.buffer().tag_table().add(&mono_tag);

    let text_scroller = gtk::ScrolledWindow::builder()
        .child(&text_view)
        .vexpand(true)
        .hexpand(true)
        .build();
    let text_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .build();
    let gutter = LineNumberGutter::new(&text_view, &text_scroller);
    text_row.append(&gutter.area);
    text_row.append(&text_scroller);
    let text_clamp = adw::Clamp::builder()
        .maximum_size(1200)
        .tightening_threshold(800)
        .child(&text_row)
        .build();
    stack.add_named(&text_clamp, Some("text"));

    let pdf_picture = gtk::Picture::builder()
        .can_shrink(true)
        .halign(gtk::Align::Center)
        .build();
    pdf_picture.add_css_class("flux-pdf-page");
    let pdf_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(PDF_GUTTER / 2)
        .margin_end(PDF_GUTTER / 2)
        .halign(gtk::Align::Fill)
        .build();
    pdf_box.append(&pdf_picture);

    let pdf_scroller = gtk::ScrolledWindow::builder()
        .child(&pdf_box)
        .vexpand(true)
        .hexpand(true)
        .build();

    stack.add_named(&pdf_scroller, Some("pdf"));

    let table_view = gtk::ColumnView::builder()
        .show_row_separators(true)
        .show_column_separators(false)
        .build();

    let table_clamp = adw::Clamp::builder()
        .maximum_size(1400)
        .tightening_threshold(1000)
        .child(&table_view)
        .build();

    let table_scroller = gtk::ScrolledWindow::builder()
        .child(&table_clamp)
        .vexpand(true)
        .hexpand(true)
        .build();

    stack.add_named(&table_scroller, Some("table"));

    let meta_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .valign(gtk::Align::Start)
        .build();

    let meta_group = adw::PreferencesGroup::builder()
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    meta_group.add(&meta_list);

    let meta_scroller = gtk::ScrolledWindow::builder()
        .child(&info_clamp(&meta_group))
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    stack.add_named(&meta_scroller, Some("meta"));

    let folder_grid = build_folder_grid(&sender);
    let folder_thumbs = FolderThumbs::new(&folder_grid);
    let folder_view = folder_grid.borrow().view.clone();
    // Keep the cells clear of the overlaid nav arrows so none ends up under one.
    folder_view.set_margin_start(FOLDER_GRID_NAV_GUTTER);
    folder_view.set_margin_end(FOLDER_GRID_NAV_GUTTER);
    let folder_scroller = gtk::ScrolledWindow::builder()
        .child(&folder_view)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .hexpand(true)
        .build();
    attach_folder_zoom_scroll(&folder_scroller);
    stack.add_named(&folder_scroller, Some("folder"));

    stack.set_visible_child_name("empty");

    // Placeholder, never inserted anywhere, so calls to `footer.set_visible`
    // in `apply` do not need to special-case the panel variants that skip it.
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let details_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .valign(gtk::Align::Start)
        .build();

    let details_group = adw::PreferencesGroup::builder()
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(12)
        .build();
    details_group.add(&details_list);

    let details_scroller = gtk::ScrolledWindow::builder()
        .child(&info_clamp(&details_group))
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let view_stack = adw::ViewStack::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    view_stack.add_titled_with_icon(
        &stack,
        Some("preview"),
        &tr("Preview"),
        "view-compact-symbolic",
    );
    view_stack.add_titled_with_icon(
        &details_scroller,
        Some("details"),
        &tr("Details"),
        "dialog-information-symbolic",
    );
    // Left unparented on purpose, the window header hosts it as part of
    // the `preview-switcher` stack page.
    let switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .halign(gtk::Align::Center)
        .build();

    let menu_btn = flat_icon_button("view-more-symbolic", &tr("Menu"));
    let actions_btn = flat_icon_button("system-run-symbolic", &tr("Contextual actions"));
    attach_menu_button(&menu_btn, sender.clone(), false);
    attach_menu_button(&actions_btn, sender.clone(), true);

    let extras: Vec<gtk::Widget> = vec![
        nav_box.clone().upcast(),
        spacer.clone().upcast(),
        wrap_btn.clone().upcast(),
        md_source_btn.clone().upcast(),
        install_font_btn.clone().upcast(),
        invert_btn.clone().upcast(),
        details_toggle.clone().upcast(),
        zoom_out.clone().upcast(),
        zoom_btn.clone().upcast(),
        zoom_in.clone().upcast(),
        playlist_btn.clone().upcast(),
        mute_btn.clone().upcast(),
        wide_btn.clone().upcast(),
        fullscreen_btn.clone().upcast(),
        mime_filter_btn.clone().upcast(),
        menu_btn.clone().upcast(),
        actions_btn.clone().upcast(),
    ];
    let header = {
        let s = sender.clone();
        let header = panel_header(
            Some("view-compact-symbolic"),
            &tr("Preview"),
            &extras,
            move || s.input(AppMsg::TogglePreview),
        );
        panel.append(&header);
        header
    };
    {
        let s = sender.clone();
        wide_btn.connect_toggled(move |b| s.input(AppMsg::SetWidePreviewDefault(b.is_active())));
    }
    {
        let s = sender.clone();
        fullscreen_btn.connect_clicked(move |_| s.input(AppMsg::TogglePreviewFullscreen));
    }
    {
        let s = sender.clone();
        mime_filter_btn.connect_toggled(move |_b| {
            // Skip the message when the active state was pushed by
            // `set_mime_filter_active` from `FluxApp`.
            let suppress =
                preview_registry::with(|h| h.state.mime_filter_suppress.get()).unwrap_or(true);
            if suppress {
                return;
            }
            s.input(AppMsg::FilterBySelectedMime);
        });
    }
    mute_btn.connect_toggled(|b| {
        PREVIEW_MUTED.store(b.is_active(), Ordering::SeqCst);
        b.set_icon_name(if b.is_active() {
            "audio-volume-muted-symbolic"
        } else {
            "audio-volume-high-symbolic"
        });
        preview_registry::with(|h| h.apply_mute());
    });
    playlist_btn.connect_toggled(move |b| {
        b.set_icon_name(if b.is_active() {
            &playlist_icon_on
        } else {
            &playlist_icon_off
        });
        preview_registry::with(|h| h.set_playlist(b.is_active()));
    });
    {
        let s = sender.clone();
        install_font_btn.connect_clicked(move |_| {
            let path = preview_registry::with(|h| h.state.path.borrow().clone()).flatten();
            if let Some(path) = path {
                s.input(AppMsg::InstallFonts(vec![path]));
            }
        });
    }
    panel.append(&banner);

    // Floating prev/next buttons over the preview content. They step the grid
    // selection so the panel follows with the adjacent item (the same thing
    // Left/Right do on the grid). Semi-transparent by default, fully opaque on
    // hover so they stay discoverable without dominating the image.
    //
    // The glyph is set through an explicit `gtk::Image::pixel_size` child
    // instead of the button's `icon_name`. GTK4 buttons with `icon_name` fold
    // the icon's `-gtk-icon-size` into their natural size before the CSS
    // `min-width`/`min-height` floor applies, so increasing `-gtk-icon-size`
    // in CSS does not grow the visible button. `pixel_size` sets an absolute
    // child size that propagates straight to the button's natural size.
    let nav_prev = gtk::Button::builder()
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .margin_start(8)
        .css_classes(["flat", "circular", "preview-nav-btn"])
        .tooltip_text(tr("Previous item"))
        .build();
    nav_prev.set_child(Some(
        &gtk::Image::builder()
            .icon_name("go-previous-symbolic")
            .pixel_size(56)
            .build(),
    ));
    let nav_next = gtk::Button::builder()
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .margin_end(8)
        .css_classes(["flat", "circular", "preview-nav-btn"])
        .tooltip_text(tr("Next item"))
        .build();
    nav_next.set_child(Some(
        &gtk::Image::builder()
            .icon_name("go-next-symbolic")
            .pixel_size(56)
            .build(),
    ));

    {
        let s = sender.clone();
        nav_prev.connect_clicked(move |_| s.input(AppMsg::SelectionStep(-1)));
    }
    {
        let s = sender.clone();
        nav_next.connect_clicked(move |_| s.input(AppMsg::SelectionStep(1)));
    }

    let view_overlay = gtk::Overlay::builder().vexpand(true).hexpand(true).build();
    view_overlay.set_child(Some(&view_stack));
    view_overlay.add_overlay(&nav_prev);
    view_overlay.add_overlay(&nav_next);

    // The arrows are only shown while the pointer is over the preview panel, so
    // they do not sit on top of the content when it is not being interacted with.
    nav_prev.set_visible(false);
    nav_next.set_visible(false);
    let hover = gtk::EventControllerMotion::new();
    {
        let (prev, next) = (nav_prev.clone(), nav_next.clone());
        hover.connect_contains_pointer_notify(move |c| {
            let inside = c.contains_pointer();
            prev.set_visible(inside);
            next.set_visible(inside);
        });
    }
    panel.add_controller(hover);

    panel.append(&view_overlay);

    // Double-click on the preview content toggles full-screen preview.
    let dbl_click = gtk::GestureClick::new();
    dbl_click.set_button(gtk::gdk::BUTTON_PRIMARY);
    dbl_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let s = sender.clone();
        dbl_click.connect_pressed(move |g, n_press, x, y| {
            if n_press != 2 {
                return;
            }
            let Some(root) = g.widget() else {
                return;
            };
            if root
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .is_some_and(|w| double_click_reserved(&w))
            {
                return;
            }
            s.input(AppMsg::TogglePreviewFullscreen);
        });
    }
    view_overlay.add_controller(dbl_click);

    let handle = PreviewPanelHandle {
        stack,
        nav_box,
        page_spin: page_spin.clone(),
        page_total,
        prev_btn: prev_btn.clone(),
        next_btn: next_btn.clone(),
        wrap_btn: wrap_btn.clone(),
        invert_btn: invert_btn.clone(),
        md_source_btn: md_source_btn.clone(),
        wide_btn: wide_btn.clone(),
        fullscreen_btn: fullscreen_btn.clone(),
        mute_btn: mute_btn.clone(),
        header,
        folder_grid,
        mime_filter_btn: mime_filter_btn.clone(),
        playlist_btn: playlist_btn.clone(),
        install_font_btn: install_font_btn.clone(),
        zoom_btn: zoom_btn.clone(),
        banner,
        image,
        image_scroller: image_scroller.clone(),
        text_view: text_view.clone(),
        text_tag,
        md_base_tag,
        pdf_scroller: pdf_scroller.clone(),
        pdf_picture: pdf_picture.clone(),
        table_view: table_view.clone(),
        meta_list,
        nopreview,
        footer,
        switcher: switcher.clone(),
        view_stack: view_stack.clone(),
        details_list,
        media_video,
        media_cover,
        audio_art_stack,
        audio_status,
        audio_btn: audio_btn.clone(),
        audio_back: audio_back.clone(),
        audio_fwd: audio_fwd.clone(),
        audio_seek: audio_seek.clone(),
        audio_vol: audio_vol.clone(),
        audio_time: audio_time.clone(),
        audio_dur: audio_dur.clone(),
        state: Rc::new(State {
            mode: Cell::new(Mode::None),
            token: Cell::new(0),
            path: RefCell::new(None),
            pdf_path: RefCell::new(None),
            pages: Cell::new(0),
            page: Cell::new(0),
            zoom: Cell::new(1.0),
            suppress: Cell::new(false),
            rendered_w: Cell::new(0),
            resize_gen: Cell::new(0),
            tex_px: Cell::new((0, 0)),
            last_image_view: Cell::new((0, 0)),
            media: RefCell::new(None),
            syntax_tags: RefCell::new(Vec::new()),
            md_cache: RefCell::new(None),
            mime_filter_suppress: Cell::new(false),
            folder_thumbs,
            folder_icon_size: Cell::new(FOLDER_GRID_ICON_SIZE),
            folder_entries: RefCell::new(Vec::new()),
            fill_gen: Cell::new(0),
            playlist: Cell::new(false),
            playlist_skip_until: Cell::new(0),
        }),
    };
    preview_registry::set_current(handle);

    // The Details toggle and the switcher drive the same stack page, each
    // listens to the other so whichever one the user touches wins.
    {
        let vs = view_stack.clone();
        details_toggle.connect_toggled(move |b| {
            let target = if b.is_active() { "details" } else { "preview" };
            vs.set_visible_child_name(target);
        });
    }
    {
        let toggle = details_toggle.clone();
        view_stack.connect_visible_child_name_notify(move |vs| {
            let on_details = vs.visible_child_name().as_deref() == Some("details");
            if toggle.is_active() != on_details {
                toggle.set_active(on_details);
            }
        });
    }

    audio_btn.connect_clicked(|_| {
        preview_registry::with(|h| h.audio_toggle());
    });
    audio_back.connect_clicked(|_| ipc_send(r#"{"command":["seek",-10,"relative"]}"#));
    audio_fwd.connect_clicked(|_| ipc_send(r#"{"command":["seek",10,"relative"]}"#));
    audio_seek.connect_change_value(|_, _, value| {
        AUDIO_SEEK_HOLD.store(glib::monotonic_time() + 500_000, Ordering::SeqCst);
        ipc_send(&format!(
            r#"{{"command":["seek",{:.3},"absolute"]}}"#,
            value.max(0.0)
        ));
        glib::Propagation::Proceed
    });
    audio_vol.connect_change_value(|_, _, value| {
        ipc_send(&format!(
            r#"{{"command":["set_property","volume",{:.0}]}}"#,
            value.clamp(0.0, 100.0)
        ));
        glib::Propagation::Proceed
    });

    prev_btn.connect_clicked(|_| {
        preview_registry::with(|h| h.go_to_page(h.state.page.get().saturating_sub(1)));
    });
    next_btn.connect_clicked(|_| {
        preview_registry::with(|h| h.go_to_page(h.state.page.get() + 1));
    });
    page_spin.connect_value_changed(|s| {
        preview_registry::with(|h| {
            if !h.state.suppress.get() {
                h.go_to_page((s.value() as usize).saturating_sub(1));
            }
        });
    });
    zoom_out.connect_clicked(|_| {
        preview_registry::with(|h| h.zoom_by(1.0 / ZOOM_STEP));
    });
    zoom_in.connect_clicked(|_| {
        preview_registry::with(|h| h.zoom_by(ZOOM_STEP));
    });
    zoom_btn.connect_clicked(|_| {
        preview_registry::with(|h| h.set_zoom(1.0));
    });
    {
        let tv = text_view.clone();
        wrap_btn.connect_toggled(move |b| {
            tv.set_wrap_mode(if b.is_active() {
                gtk::WrapMode::WordChar
            } else {
                gtk::WrapMode::None
            });
        });
    }
    {
        let pic = pdf_picture.clone();
        invert_btn.connect_toggled(move |b| {
            if b.is_active() {
                pic.add_css_class("flux-pdf-invert");
            } else {
                pic.remove_css_class("flux-pdf-invert");
            }
        });
    }
    md_source_btn.connect_toggled(|_| {
        preview_registry::with(|h| h.apply_markdown_view());
    });

    // Pan is only attached to the image and PDF scrollers, the text
    // scroller has its own caret-driven scroll behaviour and would fight
    // a drag-to-pan controller.
    for sw in [&pdf_scroller, &image_scroller] {
        attach_zoom_scroll(sw);
        attach_pan(sw);
    }
    attach_zoom_scroll(&text_scroller);

    pdf_scroller
        .hadjustment()
        .connect_notify_local(Some("page-size"), |_, _| {
            preview_registry::with(|h| h.schedule_resize());
        });
    image_scroller
        .hadjustment()
        .connect_notify_local(Some("page-size"), |_, _| {
            preview_registry::with(|h| h.schedule_resize());
        });
    image_scroller
        .vadjustment()
        .connect_notify_local(Some("page-size"), |_, _| {
            preview_registry::with(|h| h.schedule_resize());
        });

    panel.connect_unmap(|_| {
        preview_registry::with(|h| h.stop_media());
    });

    attach_context_menu(&panel, &text_view, sender.clone());

    let sender_for_resize = sender.clone();
    resizable_panel(&spec, &panel, move |w| {
        sender_for_resize.input(AppMsg::SetPreviewPanelWidth(w));
    })
}

// ─── Behaviour ────────────────────────────────────────────────────────────────

impl PreviewPanelHandle {
    /// Reflects `config.ui.wide_preview` on the header button without echoing a message back.
    pub fn set_wide_default_active(&self, on: bool) {
        if self.wide_btn.is_active() != on {
            self.wide_btn.set_active(on);
        }
    }

    /// Reflects full-screen preview on the header button (icon and tooltip).
    pub fn set_fullscreen_active(&self, on: bool) {
        // Full screen shows only the content, Esc or double-click bring the strip back.
        self.header.set_visible(!on);
        let icon = if on {
            first_available_icon(&["view-restore-symbolic", "view-fullscreen-symbolic"])
        } else {
            first_available_icon(&["view-fullscreen-symbolic", "zoom-fit-best-symbolic"])
        };
        self.fullscreen_btn.set_icon_name(&icon);
        self.fullscreen_btn.set_tooltip_text(Some(&if on {
            tr("Exit full screen (Esc)")
        } else {
            tr("Full screen")
        }));
    }

    /// Empties the folder grid and cancels its thumbnail loads, so nothing of a folder
    /// the preview has left stays in memory or keeps a worker busy.
    pub(crate) fn release_folder_grid(&self) {
        self.state.fill_gen.set(self.state.fill_gen.get() + 1);
        self.state.folder_thumbs.stop();
        let mut grid = self.folder_grid.borrow_mut();
        if !grid.is_empty() {
            grid.clear();
        }
    }

    /// Replaces the folder grid contents with `entries`.
    ///
    /// Building a cell costs an icon lookup, so thousands of them in one go would freeze
    /// the panel. The first chunk fills the viewport right away, the rest is added between
    /// frames and abandoned as soon as the preview moves on to something else.
    fn fill_folder_grid(&self, entries: &[FolderEntry]) {
        crate::hit!("fill_folder_grid");
        let cfg = crate::utils::load_config();
        let st = &self.state;
        let fill_gen = st.fill_gen.get() + 1;
        st.fill_gen.set(fill_gen);
        st.folder_thumbs.stop();
        self.folder_grid.borrow_mut().clear();
        // Image thumbnails load lazily as cells bind, `None` when switched off.
        let hook = st.folder_thumbs.begin(&cfg);

        *st.folder_entries.borrow_mut() = entries.to_vec();
        let fill = Rc::new(FolderFill {
            icon_size: st.folder_icon_size.get(),
            entries: entries.to_vec(),
            cfg,
            hook,
            next: Cell::new(0),
        });
        self.append_folder_chunk(&fill, FOLDER_FIRST_BUDGET);
        if fill.next.get() >= fill.entries.len() {
            return;
        }

        let h = self.clone();
        gtk::glib::idle_add_local(move || {
            if h.state.fill_gen.get() != fill_gen {
                return gtk::glib::ControlFlow::Break;
            }
            h.append_folder_chunk(&fill, FOLDER_CHUNK_BUDGET);
            if fill.next.get() >= fill.entries.len() {
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
    }

    /// Appends entries of `fill` to the folder grid until `budget` is used up
    /// (at least one), so a chunk never holds the main loop for a whole frame.
    fn append_folder_chunk(&self, fill: &FolderFill, budget: Duration) {
        crate::hit!("append_folder_chunk");
        let began = std::time::Instant::now();
        let start = fill.next.get();
        let cfg = &fill.cfg;
        let mut grid = self.folder_grid.borrow_mut();
        let mut end = start;
        for (idx, e) in (start as u32..).zip(&fill.entries[start..]) {
            if end > start && began.elapsed() >= budget {
                break;
            }
            end += 1;
            let icon = crate::utils::icon::get_icon_for_path(&e.path, e.is_dir);
            let label = crate::utils::helpers::format_display_label(
                &e.name,
                e.is_dir,
                &cfg.ui.hidden_extensions,
            );
            grid.append(
                FileItem::builder(e.name.clone(), e.path.clone(), icon)
                    .display_label(label)
                    .is_dir(e.is_dir)
                    .size(e.size)
                    .mtime(e.mtime)
                    .icon_size(fill.icon_size)
                    .grid_idx(idx)
                    .max_width_chars(cfg.ui.max_width_chars)
                    .grid_spacing(cfg.ui.grid_spacing)
                    .show_symlink_emblem(cfg.ui.show_symlink_emblem)
                    .scale_font_with_icons(cfg.ui.scale_font_with_icons)
                    .default_icon_size(cfg.ui.default_icon_size)
                    .disable_drag_and_drop(true)
                    .thumb_hook(fill.hook.clone())
                    .build(),
            );
        }
        fill.next.set(end);
    }

    /// Steps the folder grid icon size up or down and refills the grid, keeping the scroll position.
    fn zoom_folder_grid(&self, bigger: bool) {
        let st = &self.state;
        let old = st.folder_icon_size.get();
        let sizes: Vec<i32> = crate::ui::constants::CRISP_ICON_SIZES
            .iter()
            .copied()
            .filter(|s| (FOLDER_GRID_MIN_ICON..=FOLDER_GRID_MAX_ICON).contains(s))
            .collect();
        let idx = sizes
            .iter()
            .position(|&s| s >= old)
            .unwrap_or(sizes.len() - 1);
        let new = if bigger {
            sizes[(idx + 1).min(sizes.len() - 1)]
        } else {
            sizes[idx.saturating_sub(1)]
        };
        if new == old || st.folder_entries.borrow().is_empty() {
            return;
        }
        st.folder_icon_size.set(new);

        let scroller = self
            .stack
            .child_by_name("folder")
            .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok());
        let adj = scroller.map(|sw| sw.vadjustment());
        let scaled = adj
            .as_ref()
            .map(|a| a.value() * f64::from(new) / f64::from(old));

        let entries = st.folder_entries.borrow().clone();
        self.fill_folder_grid(&entries);

        if let (Some(adj), Some(value)) = (adj, scaled) {
            gtk::glib::timeout_add_local_once(Duration::from_millis(60), move || {
                adj.set_value(value.min((adj.upper() - adj.page_size()).max(0.0)));
            });
        }
    }

    /// Focuses the folder grid and selects its first item when nothing is selected.
    pub fn focus_folder_grid(&self) -> bool {
        if self.stack.visible_child_name().as_deref() != Some("folder") {
            return false;
        }
        let view = self.folder_grid.borrow().view.clone();
        if let Some(model) = view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            if model.n_items() > 0 && model.selection().is_empty() {
                model.select_item(0, true);
            }
        }
        view.grab_focus();
        view.has_focus()
    }

    /// True when keyboard focus is inside the folder grid.
    pub fn folder_grid_has_focus(&self) -> bool {
        let view = self.folder_grid.borrow().view.clone();
        view.root()
            .and_then(|r| r.focus())
            .is_some_and(|f| f == *view.upcast_ref::<gtk::Widget>() || f.is_ancestor(&view))
    }

    /// Reflects the ephemeral MIME filter state on the header button without
    /// echoing `FilterBySelectedMime` back into `FluxApp`.
    pub fn set_mime_filter_active(&self, on: bool) {
        if self.mime_filter_btn.is_active() == on {
            return;
        }
        self.state.mime_filter_suppress.set(true);
        self.mime_filter_btn.set_active(on);
        self.state.mime_filter_suppress.set(false);
    }

    /// Turns playlist mode on or off. A playing video stops looping so it can end.
    fn set_playlist(&self, on: bool) {
        self.state.playlist.set(on);
        self.state.playlist_skip_until.set(0);
        // `gtk::Video` is built with `loop_(true)` and pushes that onto its stream,
        // so the widget property is what has to change for the video to end.
        self.media_video.set_loop(!on);
        if on {
            if let Some(mf) = self.state.media.borrow().as_ref() {
                mf.set_loop(false);
            }
        }
    }

    /// Called when the current video/audio reached its end: selects the next item.
    ///
    /// Non-media items that follow (covers, logs, folders) are skipped, see `apply`.
    /// Stepping clamps at the last item, so the playlist simply stops there.
    fn playlist_advance(&self) {
        if !self.state.playlist.get() {
            return;
        }
        self.state
            .playlist_skip_until
            .set(glib::monotonic_time() + PLAYLIST_SKIP_WINDOW_US);
        if let Some(s) = crate::model::SENDER.get() {
            let _ = s.send(AppMsg::SelectionStep(1));
        }
    }

    pub fn apply(&self, content: &PreviewContent) {
        crate::hit!("apply");
        let st = &self.state;
        st.token.set(st.token.get() + 1);
        self.stop_media();
        self.banner.set_revealed(false);

        // Leaving a folder (another file, `Loading` while arrowing, an error): free its
        // grid and drop its thumbnail loads. A new folder refills the grid below.
        if !matches!(content, PreviewContent::Folder { .. }) {
            self.release_folder_grid();
        }

        // The playlist toggle only makes sense for playable media. `Loading` keeps the
        // current state so the header does not flicker between consecutive tracks.
        match content {
            PreviewContent::Media { .. } => {
                self.playlist_btn.set_visible(true);
                self.mute_btn.set_visible(true);
                st.playlist_skip_until.set(0);
            }
            PreviewContent::Loading => {}
            _ => {
                self.playlist_btn.set_visible(false);
                self.mute_btn.set_visible(false);
                // Reached by a playlist auto-advance: not playable, move on to the next item.
                if st.playlist.get() && glib::monotonic_time() < st.playlist_skip_until.get() {
                    self.playlist_advance();
                }
            }
        }

        match content {
            PreviewContent::Loading => {
                self.set_mode(Mode::None);
                self.footer.set_visible(false);
                self.switcher.set_visible(false);
                self.view_stack.set_visible_child_name("preview");
                self.stack.set_visible_child_name("loading");
            }

            PreviewContent::Image { tex, info } => {
                self.show_info(info);
                self.set_mode(Mode::Image);
                let is_font = crate::infra::fonts::is_font_path(&info.path);
                let installed = is_font && crate::infra::fonts::is_installed(&info.path);
                self.install_font_btn.set_visible(is_font);
                self.install_font_btn.set_sensitive(!installed);
                self.install_font_btn.set_tooltip_text(Some(&if installed {
                    tr("Already installed")
                } else {
                    tr("Install font")
                }));
                st.zoom.set(1.0);
                self.sync_zoom_label();
                st.tex_px.set((tex.width(), tex.height()));
                st.last_image_view.set((0, 0));
                self.image.set_paintable(Some(tex));
                self.stack.set_visible_child_name("image");
                let h = self.clone();
                gtk::glib::idle_add_local_once(move || h.relayout_image());
            }

            PreviewContent::Pdf {
                pages,
                info,
                render_path,
            } => {
                self.show_info(info);
                *st.pdf_path.borrow_mut() = Some(render_path.clone());
                self.set_mode(Mode::Pdf);
                st.zoom.set(1.0);
                st.page.set(0);
                st.pages.set((*pages).max(1));
                st.rendered_w.set(0);
                self.sync_zoom_label();
                self.page_spin.set_range(1.0, (*pages).max(1) as f64);
                self.page_total.set_label(&format!("/ {}", (*pages).max(1)));
                self.sync_nav();
                self.pdf_scroller.vadjustment().set_value(0.0);
                self.pdf_scroller.hadjustment().set_value(0.0);
                self.stack.set_visible_child_name("loading");
                self.render_pdf();
            }

            PreviewContent::Text {
                head,
                truncated,
                hex,
                info,
                highlighted,
                language,
            } => {
                self.show_info(info);
                self.set_mode(Mode::Text);
                st.zoom.set(1.0);
                self.sync_zoom_label();
                self.text_tag.set_scale(1.0);
                self.wrap_btn.set_sensitive(!*hex);
                self.text_view
                    .set_wrap_mode(if !*hex && self.wrap_btn.is_active() {
                        gtk::WrapMode::WordChar
                    } else {
                        gtk::WrapMode::None
                    });

                let buf = self.text_view.buffer();
                buf.set_text(head);

                // Monospace first, then syntax tags on top: the syntax tags
                // override fg/weight/style, not family, so the family has
                // to be applied to the whole buffer beforehand.
                let (bs, be) = buf.bounds();
                if let Some(mono) = buf.tag_table().lookup("flux-mono") {
                    buf.apply_tag(&mono, &bs, &be);
                }

                // syntect emits byte offsets, GtkTextBuffer iterators are
                // char-indexed.
                let ranges = self.rebuild_syntax_tags(highlighted);
                if !ranges.is_empty() {
                    let map = byte_to_char_map(head);
                    for (name, range) in ranges {
                        let start = buf.iter_at_offset(map[range.start.min(head.len())]);
                        let end = buf.iter_at_offset(map[range.end.min(head.len())]);
                        buf.apply_tag_by_name(&name, &start, &end);
                    }
                }
                let (s, e) = buf.bounds();
                buf.apply_tag(&self.text_tag, &s, &e);

                if *hex {
                    self.banner
                        .set_title(&tr("Binary file - showing the first bytes as hex."));
                    self.banner.set_revealed(true);
                } else if *truncated {
                    self.banner
                        .set_title(&tr("Preview truncated - showing the start of the file."));
                    self.banner.set_revealed(true);
                } else if let Some(lang) = language {
                    self.banner
                        .set_title(&format!("{} {}", tr("Language:"), lang));
                    self.banner.set_revealed(true);
                }

                self.stack.set_visible_child_name("text");
            }

            PreviewContent::Markdown {
                rendered,
                styles,
                source,
                info,
            } => {
                self.show_info(info);
                self.set_mode(Mode::Markdown);
                st.zoom.set(1.0);
                self.sync_zoom_label();
                self.text_tag.set_scale(1.0);
                *self.state.md_cache.borrow_mut() = Some((rendered.clone(), styles.clone()));
                unsafe {
                    self.text_view.set_data("flux-md-source", source.clone());
                }
                self.md_source_btn.set_active(false);
                self.apply_markdown_view();
                self.stack.set_visible_child_name("text");
            }

            PreviewContent::Table { data, info } => {
                self.show_info(info);
                self.set_mode(Mode::Table);
                let store = preview_table::build_store(data);
                let selection = gtk::NoSelection::new(Some(store));
                self.table_view.set_model(Some(&selection));
                self.table_view.set_hexpand(true);
                self.table_view.set_vexpand(true);
                preview_table::build_columns(&self.table_view, data);
                if data.truncated {
                    self.banner
                        .set_title(&tr("Table truncated - showing the first rows/columns."));
                    self.banner.set_revealed(true);
                }
                self.stack.set_visible_child_name("table");
            }

            PreviewContent::Folder {
                entries,
                more,
                info,
            } => {
                self.show_info(info);
                self.set_mode(Mode::None);
                self.fill_folder_grid(entries);
                if *more > 0 {
                    self.banner
                        .set_title(&tr("Folder truncated - showing the first items only."));
                    self.banner.set_revealed(true);
                }
                self.stack.set_visible_child_name("folder");
            }

            PreviewContent::Media {
                path,
                video,
                cover,
                info,
            } => {
                self.show_info(info);
                self.set_mode(Mode::None);

                if *video {
                    let mf = gtk::MediaFile::new();
                    // GStreamer sometimes plays video silently until the
                    // volume is nudged. Toggle mute and volume to force the
                    // audio sink to negotiate.
                    fn kick_audio(m: &gtk::MediaFile) {
                        m.set_muted(true);
                        m.set_muted(false);
                        m.set_volume(0.99);
                        m.set_volume(1.0);
                        m.set_muted(PREVIEW_MUTED.load(Ordering::SeqCst));
                    }
                    mf.connect_prepared_notify(|m| {
                        if m.is_prepared() {
                            kick_audio(m);
                            m.play();
                            let m = m.clone();
                            gtk::glib::timeout_add_local_once(
                                Duration::from_millis(250),
                                move || {
                                    // A selection change may have torn the
                                    // pipeline down in the meantime, only
                                    // kick if this file is still the one in
                                    // `state.media`.
                                    let still_current = preview_registry::with(|h| {
                                        h.state
                                            .media
                                            .borrow()
                                            .as_ref()
                                            .map(|cur| cur == &m)
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false);
                                    if still_current && m.is_prepared() {
                                        kick_audio(&m);
                                    }
                                },
                            );
                        }
                    });
                    mf.connect_error_notify(|m| {
                        if let Some(err) = m.error() {
                            let msg = err.message().to_string();
                            preview_registry::with(|h| h.apply(&PreviewContent::Error(msg)));
                        }
                    });
                    // `gtk::Video` loops by default and a looping stream never reports `ended`,
                    // so also watch the playback position: reaching the end, or wrapping around
                    // from it, means the file finished.
                    let last_ts = Rc::new(Cell::new(0_i64));
                    let fired = Rc::new(Cell::new(false));
                    mf.connect_timestamp_notify(move |m| {
                        let ts = m.timestamp();
                        let prev = last_ts.replace(ts);
                        let dur = m.duration();
                        if dur <= 0 || fired.get() {
                            return;
                        }
                        let near_end = ts >= dur - PLAYLIST_END_SLACK_US;
                        let wrapped = prev >= dur - PLAYLIST_WRAP_ZONE_US && ts + 1_000_000 < prev;
                        if !(near_end || wrapped) {
                            return;
                        }
                        let still_current = preview_registry::with(|h| {
                            h.state.playlist.get()
                                && h.state
                                    .media
                                    .borrow()
                                    .as_ref()
                                    .map(|cur| cur == m)
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false);
                        if still_current {
                            fired.set(true);
                            preview_registry::with(|h| h.playlist_advance());
                        }
                    });
                    mf.connect_ended_notify(|m| {
                        if !m.is_ended() {
                            return;
                        }
                        let current = preview_registry::with(|h| {
                            h.state
                                .media
                                .borrow()
                                .as_ref()
                                .map(|cur| cur == m)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                        if current {
                            preview_registry::with(|h| h.playlist_advance());
                        }
                    });
                    mf.set_file(Some(&gtk::gio::File::for_path(path)));
                    // Set the widget's loop before attaching, it applies it to the stream.
                    self.media_video.set_loop(!st.playlist.get());
                    self.media_video.set_media_stream(Some(&mf));
                    if st.playlist.get() {
                        mf.set_loop(false);
                    }
                    *self.state.media.borrow_mut() = Some(mf);
                    self.stack.set_visible_child_name("media-video");
                } else {
                    if AUDIO_MPV_PID.load(Ordering::SeqCst) != 0 {
                        stop_audio_mpv();
                    }
                    self.audio_reset_controls();
                    self.audio_art_stack
                        .set_visible_child_name(if cover.is_some() { "cover" } else { "icon" });
                    if let Some(tex) = cover {
                        self.media_cover.set_paintable(Some(tex));
                    }

                    if glib::find_program_in_path("mpv").is_none() {
                        self.audio_status.set_label(&tr(
                            "mpv is not installed. Install it to enable audio preview, or use Open.",
                        ));
                        self.audio_btn.set_sensitive(false);
                    } else {
                        self.audio_status.set_label(&tr("Ready."));
                        self.audio_btn.set_sensitive(true);
                    }
                    self.audio_btn.set_label(&tr("Play"));

                    self.stack.set_visible_child_name("media-audio");

                    // Short delay so a fast arrow-key pass does not spawn
                    // mpv for files the user is already scrolling past.
                    let token = st.token.get();
                    let h = self.clone();
                    gtk::glib::timeout_add_local_once(Duration::from_millis(300), move || {
                        if h.state.token.get() == token && h.stack.is_mapped() {
                            h.start_audio();
                        }
                    });
                }
            }

            PreviewContent::Metadata { info } => {
                self.show_info(info);
                self.set_mode(Mode::None);
                if info.extra.is_empty() {
                    self.nopreview
                        .set_icon_name(Some("text-x-generic-symbolic"));
                    self.nopreview.set_title(&tr("No preview available"));
                    self.nopreview.set_description(Some(&info.mime));
                    self.stack.set_visible_child_name("nopreview");
                } else {
                    clear_list(&self.meta_list);
                    for (k, v) in &info.extra {
                        push_row(&self.meta_list, k, v);
                    }
                    self.stack.set_visible_child_name("meta");
                }
            }

            PreviewContent::Error(msg) => {
                self.set_mode(Mode::None);
                self.footer.set_visible(false);
                self.switcher.set_visible(false);
                self.view_stack.set_visible_child_name("preview");
                self.nopreview.set_icon_name(Some("dialog-error-symbolic"));
                self.nopreview.set_title(&tr("Preview failed"));
                self.nopreview.set_description(Some(msg));
                self.stack.set_visible_child_name("nopreview");
            }
        }
    }

    fn apply_markdown_view(&self) {
        crate::hit!("apply_markdown_view");
        let show_source = self.md_source_btn.is_active();
        let source: Option<String> = unsafe {
            self.text_view
                .data::<String>("flux-md-source")
                .map(|p| p.as_ref().clone())
        };
        let Some(source) = source else { return };

        let buf = self.text_view.buffer();
        if show_source {
            buf.set_text(&source);
            for name in self.state.syntax_tags.borrow_mut().drain(..) {
                if let Some(tag) = buf.tag_table().lookup(&name) {
                    buf.tag_table().remove(&tag);
                }
            }
            let (s, e) = buf.bounds();
            buf.apply_tag(&self.text_tag, &s, &e);
        } else {
            let cache = self.state.md_cache.borrow();
            let Some((rendered, styles)) = cache.as_ref() else {
                return;
            };
            buf.set_text(rendered);

            // Baseline family first so unstyled runs between styled spans
            // do not fall back to the surrounding widget's family, the
            // per-range tags then override weight, style and family where
            // they apply, turning code spans monospace and headings bold.
            let (bs, be) = buf.bounds();
            buf.apply_tag(&self.md_base_tag, &bs, &be);

            // pulldown-cmark ranges are byte offsets, GtkTextBuffer wants chars.
            let ranges = self.rebuild_markdown_tags(styles);
            let map = byte_to_char_map(rendered);
            for (name, range) in ranges {
                let start = buf.iter_at_offset(map[range.start.min(rendered.len())]);
                let end = buf.iter_at_offset(map[range.end.min(rendered.len())]);
                buf.apply_tag_by_name(&name, &start, &end);
            }
            let (s, e) = buf.bounds();
            buf.apply_tag(&self.text_tag, &s, &e);
        }
    }

    /// Enables or disables every audio control except the play button.
    fn audio_set_interactive(&self, on: bool) {
        self.audio_seek.set_sensitive(on);
        self.audio_vol.set_sensitive(on);
        self.audio_back.set_sensitive(on);
        self.audio_fwd.set_sensitive(on);
    }

    /// Returns the audio controls to their idle state.
    fn audio_reset_controls(&self) {
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
    fn audio_toggle(&self) {
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

    fn start_audio(&self) {
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

    pub fn stop_media(&self) {
        self.state.token.set(self.state.token.get() + 1);

        let mf = self.state.media.borrow_mut().take();
        if let Some(mf) = mf {
            mf.pause();
            self.media_video.set_media_stream(None::<&gtk::MediaStream>);
            mf.clear();
        } else {
            self.media_video.set_media_stream(None::<&gtk::MediaStream>);
        }

        if AUDIO_MPV_PID.load(Ordering::SeqCst) != 0 {
            stop_audio_mpv();
            self.audio_reset_controls();
            self.audio_status.set_label(&tr("Stopped."));
        }
    }

    /// Pushes the mute state to the playing video and the mpv instance.
    fn apply_mute(&self) {
        let muted = PREVIEW_MUTED.load(Ordering::SeqCst);
        if let Some(mf) = self.state.media.borrow().as_ref() {
            mf.set_muted(muted);
            if !muted {
                // GStreamer's sink can stay silent after unmuting until the volume changes.
                mf.set_volume(0.99);
                mf.set_volume(1.0);
            }
        }
        ipc_send(&format!(r#"{{"command":["set_property","mute",{muted}]}}"#));
    }

    /// Unmutes, called when the preview panel closes.
    pub fn reset_mute(&self) {
        if self.mute_btn.is_active() {
            self.mute_btn.set_active(false);
        }
    }

    fn set_mode(&self, mode: Mode) {
        self.state.mode.set(mode);
        self.nav_box.set_visible(mode == Mode::Pdf);
        self.invert_btn.set_visible(mode == Mode::Pdf);
        self.wrap_btn
            .set_visible(mode == Mode::Text || mode == Mode::Markdown);
        self.md_source_btn.set_visible(mode == Mode::Markdown);
        self.install_font_btn.set_visible(false);
    }

    fn show_info(&self, info: &FileInfo) {
        crate::hit!("show_info");
        clear_list(&self.details_list);
        push_row(&self.details_list, &tr("Name"), &info.name);
        push_row(&self.details_list, &tr("Type"), &info.mime);
        let virtual_path = info
            .path
            .to_string_lossy()
            .starts_with(crate::infra::archive::ARCHIVE_URI);
        if info.size > 0 || !virtual_path {
            push_row(
                &self.details_list,
                &tr("Size"),
                &format!("{} ({} B)", glib::format_size(info.size), info.size),
            );
        }
        if info.mtime > 0 {
            push_row(
                &self.details_list,
                &tr("Modified"),
                &glib::DateTime::from_unix_local(info.mtime)
                    .ok()
                    .and_then(|d| d.format("%Y-%m-%d %H:%M").ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
            );
        }
        for (k, v) in &info.extra {
            push_row(&self.details_list, k, v);
        }
        if let Some(dir) = info.path.parent() {
            push_row(&self.details_list, &tr("Location"), &dir.to_string_lossy());
        }

        *self.state.path.borrow_mut() = Some(info.path.clone());
        self.footer.set_visible(true);
        self.switcher.set_visible(true);
    }

    // ── Zoom ────────────────────────────────────────────────────────────────

    fn zoom_by(&self, factor: f64) {
        self.set_zoom(self.state.zoom.get() * factor);
    }

    fn set_zoom(&self, z: f64) {
        let z = z.clamp(ZOOM_MIN, ZOOM_MAX);
        let z = if (z - 1.0).abs() < 0.03 { 1.0 } else { z };
        self.state.zoom.set(z);
        self.sync_zoom_label();
        match self.state.mode.get() {
            Mode::Pdf => self.render_pdf(),
            Mode::Image => self.relayout_image(),
            Mode::Text | Mode::Markdown => self.text_tag.set_scale(z),
            Mode::Table | Mode::None => {}
        }
    }

    fn sync_zoom_label(&self) {
        self.zoom_btn.set_label(&format!(
            "{}%",
            (self.state.zoom.get() * 100.0).round() as i32
        ));
    }

    // ── Image ───────────────────────────────────────────────────────────────

    fn relayout_image(&self) {
        let (tw, th) = self.state.tex_px.get();
        let (vw, vh) = (self.image_scroller.width(), self.image_scroller.height());
        if tw <= 0 || th <= 0 || vw <= 0 || vh <= 0 {
            return;
        }
        self.state.last_image_view.set((vw, vh));
        let sf = self.stack.scale_factor().max(1) as f64;
        let (lw, lh) = (tw as f64 / sf, th as f64 / sf);
        let fit = (vw as f64 / lw).min(vh as f64 / lh).min(1.0);
        let z = self.state.zoom.get();
        self.image.set_size_request(
            (lw * fit * z).round().max(1.0) as i32,
            (lh * fit * z).round().max(1.0) as i32,
        );
    }

    // ── PDF ─────────────────────────────────────────────────────────────────

    fn sync_nav(&self) {
        let st = &self.state;
        st.suppress.set(true);
        self.page_spin.set_value((st.page.get() + 1) as f64);
        st.suppress.set(false);
        self.prev_btn.set_sensitive(st.page.get() > 0);
        self.next_btn
            .set_sensitive(st.page.get() + 1 < st.pages.get());
    }

    fn go_to_page(&self, idx: usize) {
        let st = &self.state;
        let pages = st.pages.get();
        if pages == 0 {
            return;
        }
        let idx = idx.min(pages - 1);
        if idx == st.page.get() {
            self.sync_nav();
            return;
        }
        st.page.set(idx);
        self.sync_nav();
        self.pdf_scroller.vadjustment().set_value(0.0);
        self.render_pdf();
    }

    fn schedule_resize(&self) {
        let st = &self.state;
        let gen = st.resize_gen.get() + 1;
        st.resize_gen.set(gen);
        let h = self.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(160), move || {
            if h.state.resize_gen.get() != gen {
                return;
            }
            match h.state.mode.get() {
                Mode::Image => {
                    let view = (h.image_scroller.width(), h.image_scroller.height());
                    if view != h.state.last_image_view.get() {
                        h.relayout_image();
                    }
                }
                Mode::Pdf => {
                    let want = h.desired_pdf_logical_w();
                    if want > 0 && (want - h.state.rendered_w.get()).abs() > 2 {
                        h.render_pdf();
                    }
                }
                _ => {}
            }
        });
    }

    fn desired_pdf_logical_w(&self) -> i32 {
        let vw = self.stack.width();
        if vw <= 5 {
            return 0;
        }
        // Render at 50% of the available width so there is breathing room
        // for the navigation arrows, but leave it to the zoom multiplier to
        // scale up if the user wants it larger.
        ((vw as f64) * 0.5 * self.state.zoom.get())
            .round()
            .max(32.0) as i32
    }

    fn render_pdf(&self) {
        let token = self.state.token.get() + 1;
        self.state.token.set(token);
        self.render_pdf_inner(token, 0);
    }

    fn render_pdf_inner(&self, token: u64, attempt: u32) {
        crate::hit!("render_pdf_inner");
        if self.state.token.get() != token {
            return;
        }
        let Some(path) = self.state.pdf_path.borrow().clone() else {
            return;
        };

        let logical_w = self.desired_pdf_logical_w();
        if logical_w <= 0 {
            // The stack has no allocation yet, retry briefly while the
            // revealer animates in.
            if attempt < 40 {
                let h = self.clone();
                gtk::glib::timeout_add_local_once(Duration::from_millis(50), move || {
                    h.render_pdf_inner(token, attempt + 1);
                });
            }
            return;
        }

        let sf = self.stack.scale_factor().max(1);
        let target_px = (logical_w * sf).min(MAX_PDF_PX);
        let page = self.state.page.get();

        // Poppler rendering is synchronous and CPU-bound, keep it off the
        // GTK main thread.
        let job = gtk::gio::spawn_blocking(move || render_pdf_raw(&path, page, target_px));
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let raw = job.await.ok().flatten();
            if this.state.token.get() != token {
                return;
            }
            let Some(raw) = raw else {
                this.apply(&PreviewContent::Error(tr("Could not render this page.")));
                return;
            };
            let bytes = glib::Bytes::from_owned(raw.bytes);
            let tex = gtk::gdk::MemoryTexture::new(
                raw.w,
                raw.h,
                CAIRO_ARGB32,
                &bytes,
                raw.stride as usize,
            );
            this.pdf_picture.set_paintable(Some(&tex));
            this.pdf_picture.set_size_request(
                (raw.w as f64 / sf as f64).round() as i32,
                (raw.h as f64 / sf as f64).round() as i32,
            );
            this.state.rendered_w.set(logical_w);
            this.stack.set_visible_child_name("pdf");
        });
    }
}

// ─── PDF rasteriser (worker thread) ──────────────────────────────────────────

struct RawPage {
    bytes: Vec<u8>,
    w: i32,
    h: i32,
    stride: i32,
}

fn render_pdf_raw(path: &Path, page_index: usize, target_w: i32) -> Option<RawPage> {
    crate::hit!("render_pdf_raw");
    let doc = poppler::PopplerDocument::new_from_file(path, None).ok()?;
    let page = doc.get_page(page_index)?;
    let (pw, ph) = page.get_size();
    if pw <= 0.0 || ph <= 0.0 {
        return None;
    }

    let mut scale = target_w as f64 / pw;
    if ph * scale > MAX_PDF_PX_H as f64 {
        scale = MAX_PDF_PX_H as f64 / ph;
    }
    let rw = (pw * scale).round().max(1.0) as i32;
    let rh = (ph * scale).round().max(1.0) as i32;

    let mut surface =
        poppler::cairo::ImageSurface::create(poppler::cairo::Format::ARgb32, rw, rh).ok()?;
    {
        let cx = poppler::cairo::Context::new(&surface).ok()?;
        cx.set_source_rgb(1.0, 1.0, 1.0);
        cx.paint().ok()?;
        cx.scale(rw as f64 / pw, rh as f64 / ph);
        page.render(&cx);
    }
    surface.flush();

    let stride = surface.stride();
    let bytes = surface.data().ok()?.to_vec();
    Some(RawPage {
        bytes,
        w: rw,
        h: rh,
        stride,
    })
}

// ─── Small widget helpers ─────────────────────────────────────────────────────

fn install_css() {
    thread_local! {
        static DONE: Cell<bool> = const { Cell::new(false) };
    }
    if DONE.with(|d| d.replace(true)) {
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".flux-pdf-page { box-shadow: 0 1px 6px alpha(black, 0.35); }\n\
         .flux-pdf-invert { filter: invert(1) hue-rotate(180deg); }\n\
         .flux-preview-gutter { background-color: alpha(currentColor, 0.03); }\n\
         .preview-nav-btn {\n\
           min-width: 3.2em; min-height: 3.2em;\n\
           background-color: alpha(black, 0.35);\n\
           color: alpha(white, 0.85);\n\
           opacity: 0.55;\n\
           transition: opacity 150ms ease, background-color 150ms ease;\n\
         }\n\
         .preview-nav-btn image {\n\
           -gtk-icon-size: 1.8em;\n\
         }\n\
         .preview-nav-btn:hover {\n\
           opacity: 1;\n\
           background-color: alpha(black, 0.6);\n\
         }\n\
         .preview-nav-btn:active {\n\
           opacity: 1;\n\
           background-color: alpha(black, 0.75);\n\
         }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// First icon of `names` that the current theme has, so a missing glyph never leaves a blank button.
fn first_available_icon(names: &[&str]) -> String {
    let theme = gtk::gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d));
    names
        .iter()
        .find(|n| theme.as_ref().is_some_and(|t| t.has_icon(n)))
        .or(names.last())
        .map(|n| n.to_string())
        .unwrap_or_default()
}

fn flat_icon_button(icon: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tip)
        .css_classes(["flat"])
        .build()
}

/// True when the widget (or an ancestor) handles double-click itself: text, buttons, sliders, entries.
fn double_click_reserved(widget: &gtk::Widget) -> bool {
    crate::hit!("double_click_reserved");
    let mut cur = Some(widget.clone());
    while let Some(w) = cur {
        if w.is::<gtk::TextView>()
            || w.is::<gtk::Button>()
            || w.is::<gtk::Range>()
            || w.is::<gtk::Scrollbar>()
            || w.is::<gtk::Editable>()
            || w.is::<gtk::GridView>()
        {
            return true;
        }
        cur = w.parent();
    }
    false
}

/// Default icon size of the folder grid cells.
const FOLDER_GRID_ICON_SIZE: i32 = 96;
/// Side room the folder grid leaves for the nav arrows (8px margin + button width).
const FOLDER_GRID_NAV_GUTTER: i32 = 88;
/// Smallest and largest icon size Ctrl+scroll can reach in the folder grid.
const FOLDER_GRID_MIN_ICON: i32 = 32;
const FOLDER_GRID_MAX_ICON: i32 = 256;
/// Time spent creating cells synchronously when a folder is shown (fills the viewport).
const FOLDER_FIRST_BUDGET: Duration = Duration::from_millis(16);
/// Time spent creating cells per idle callback afterwards, well under one frame.
const FOLDER_CHUNK_BUDGET: Duration = Duration::from_millis(6);

/// A folder grid fill in progress, see `fill_folder_grid`.
struct FolderFill {
    icon_size: i32,
    entries: Vec<FolderEntry>,
    cfg: crate::model::Config,
    hook: Option<crate::ui::ThumbHook>,
    next: Cell<usize>,
}

/// Builds the interactive grid shown when the selected item is a directory.
fn build_folder_grid(
    sender: &AsyncComponentSender<FluxApp>,
) -> Rc<RefCell<TypedGridView<FileItem, gtk::MultiSelection>>> {
    crate::hit!("build_folder_grid");
    let single_click = crate::utils::load_config().ui.single_click;
    let grid = Rc::new(RefCell::new(
        TypedGridView::<FileItem, gtk::MultiSelection>::new(),
    ));
    let view = grid.borrow().view.clone();
    view.set_enable_rubberband(true);
    view.set_single_click_activate(single_click);
    view.set_min_columns(1);
    view.set_max_columns(20);
    view.set_hexpand(true);
    view.set_vexpand(true);

    // Enter or click: folders open in the main grid, files open with the default app.
    // Files inside an archive go through `ActivateItems` so they are extracted first.
    {
        let weak = Rc::downgrade(&grid);
        let s = sender.clone();
        view.connect_activate(move |_, position| {
            let Some(grid) = weak.upgrade() else {
                return;
            };
            let target = grid.borrow().get(position).map(|w| {
                let item = w.borrow();
                (item.path.clone(), item.is_dir)
            });
            match target {
                Some((path, true)) => s.input(AppMsg::Navigate(path)),
                Some((path, false))
                    if path
                        .to_string_lossy()
                        .starts_with(crate::infra::archive::ARCHIVE_URI) =>
                {
                    s.input(AppMsg::ActivateItems(vec![(path, false)]))
                }
                Some((path, false)) => FluxApp::open_file(path),
                None => {}
            }
        });
    }

    // Esc hands the keyboard back to the main grid.
    {
        let s = sender.clone();
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape || key == gtk::gdk::Key::Tab {
                s.input(AppMsg::TogglePreviewGridFocus);
                return gtk::glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        view.add_controller(keys);
    }

    // Read-only for now: the cell context menu acts on the main tab, so claim right-click here.
    let block_menu = gtk::GestureClick::new();
    block_menu.set_button(gtk::gdk::BUTTON_SECONDARY);
    block_menu.set_propagation_phase(gtk::PropagationPhase::Capture);
    block_menu.connect_pressed(|g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
    });
    view.add_controller(block_menu);

    grid
}

/// Caps an info list's width and centres it so wide panels keep it clear of the nav arrows.
fn info_clamp(child: &impl IsA<gtk::Widget>) -> adw::Clamp {
    adw::Clamp::builder()
        .maximum_size(INFO_MAX_WIDTH)
        .tightening_threshold(INFO_MAX_WIDTH)
        .child(child)
        .build()
}

/// Opens the primary or secondary menu for the previewed file, anchored under `btn`.
fn attach_menu_button(btn: &gtk::Button, sender: AsyncComponentSender<FluxApp>, secondary: bool) {
    btn.connect_clicked(move |b| {
        let (x, y) = crate::utils::helpers::to_popover_space(
            b.upcast_ref(),
            f64::from(b.width()) / 2.0,
            f64::from(b.height()),
        );
        // `None` makes the handler resolve the selected file, which the preview follows.
        sender.input(if secondary {
            AppMsg::PrepareSecondaryMenu { x, y, path: None }
        } else {
            AppMsg::PrepareContextMenu(x, y, None)
        });
    });
}

/// Right-click opens the primary menu for the previewed (selected) file.
fn attach_context_menu(
    panel: &gtk::Box,
    text_view: &gtk::TextView,
    sender: AsyncComponentSender<FluxApp>,
) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

    let tv = text_view.clone();
    gesture.connect_pressed(move |g, _, x, y| {
        let Some(panel) = g.widget() else { return };

        // Ctrl+right-click belongs to the window-level secondary-menu gesture.
        let ctrl = g.current_event().is_some_and(|e| {
            e.modifier_state()
                .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        });

        if ctrl || crate::utils::load_config().ui.panel_menu_requires_ctrl {
            return;
        }

        // Keep the native copy menu when right-clicking selected text.
        let on_text = panel.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|w| {
            w.upcast_ref::<gtk::Widget>() == tv.upcast_ref::<gtk::Widget>() || w.is_ancestor(&tv)
        });
        if on_text && tv.buffer().has_selection() {
            return;
        }

        g.set_state(gtk::EventSequenceState::Claimed);
        let (rx, ry) = crate::utils::helpers::to_popover_space(&panel, x, y);
        // `None` makes the handler resolve the selected file, which the preview follows.
        sender.input(AppMsg::PrepareContextMenu(rx, ry, None));
    });
    panel.add_controller(gesture);
}

fn attach_zoom_scroll(sw: &gtk::ScrolledWindow) {
    let ctl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
    ctl.connect_scroll(|c, _dx, dy| {
        if !c
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        preview_registry::with(|h| h.zoom_by(if dy < 0.0 { ZOOM_STEP } else { 1.0 / ZOOM_STEP }));
        glib::Propagation::Stop
    });
    sw.add_controller(ctl);
}

fn attach_folder_zoom_scroll(sw: &gtk::ScrolledWindow) {
    let ctl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
    ctl.connect_scroll(|c, _dx, dy| {
        if !c
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        preview_registry::with(|h| h.zoom_folder_grid(dy < 0.0));
        glib::Propagation::Stop
    });
    sw.add_controller(ctl);
}

fn attach_pan(sw: &gtk::ScrolledWindow) {
    let drag = gtk::GestureDrag::new();
    drag.set_button(gtk::gdk::BUTTON_PRIMARY);
    let origin = Rc::new(Cell::new((0.0_f64, 0.0_f64)));
    {
        let sw = sw.clone();
        let origin = origin.clone();
        drag.connect_drag_begin(move |_, _, _| {
            origin.set((sw.hadjustment().value(), sw.vadjustment().value()));
        });
    }
    {
        let sw = sw.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let (ox, oy) = origin.get();
            sw.hadjustment().set_value(ox - dx);
            sw.vadjustment().set_value(oy - dy);
        });
    }
    sw.add_controller(drag);
}

fn clear_list(l: &gtk::ListBox) {
    while let Some(c) = l.first_child() {
        l.remove(&c);
    }
}

fn push_row(l: &gtk::ListBox, key: &str, value: &str) {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(key))
        .subtitle(glib::markup_escape_text(value))
        .build();
    l.append(&row);
}

/// Byte offsets to char offsets. `GtkTextBuffer::iter_at_offset` counts
/// chars, but `syntect` and `pulldown-cmark` hand back byte ranges.
fn byte_to_char_map(text: &str) -> Vec<i32> {
    crate::hit!("byte_to_char_map");
    let mut map = vec![0i32; text.len() + 1];
    let mut chars = 0i32;
    for (i, c) in text.char_indices() {
        for slot in map.iter_mut().skip(i).take(c.len_utf8()) {
            *slot = chars;
        }
        chars += 1;
    }
    map[text.len()] = chars;
    map
}
