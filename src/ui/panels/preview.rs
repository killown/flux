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
use super::preview_gutter::LineNumberGutter;
use super::preview_markdown::{MdStyle, StyledRange};
use super::preview_registry;
use super::preview_syntax::HighlightedRange;
use super::preview_table::{self, TableData};
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
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
    Listing {
        rows: Vec<(String, String)>,
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

fn stop_audio_mpv() {
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
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let mpv = glib::find_program_in_path("mpv").ok_or_else(|| {
        "mpv not installed - audio preview requires the `mpv` binary on PATH".to_string()
    })?;

    let mut cmd = Command::new(mpv);
    cmd.args([
        "--no-video",
        "--no-terminal",
        "--no-mute",
        "--audio-device=auto",
        "--ao=pipewire,pulse,alsa",
        "--msg-level=all=warn",
        "--",
    ])
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
            std::thread::spawn(move || {
                let status = child.wait();
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
                            h.audio_btn.set_label(&tr("Play"));
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
    pub(crate) state: Rc<State>,
}

impl PreviewPanelHandle {
    fn rebuild_syntax_tags(&self, ranges: &[HighlightedRange]) -> Vec<(String, Range<usize>)> {
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
        let table = self.text_view.buffer().tag_table();
        for name in self.state.syntax_tags.borrow_mut().drain(..) {
            if let Some(tag) = table.lookup(&name) {
                table.remove(&tag);
            }
        }

        let body_family = body_font_family();

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
                    .background("#2a2a2a")
                    .foreground("#e5c07b"),
                MdStyle::CodeBlock => b.family("monospace").foreground("#98c379"),
                MdStyle::Link => b
                    .family(&body_family)
                    .foreground("#61afef")
                    .underline(gtk::pango::Underline::Single),
                MdStyle::BlockQuote => b
                    .family(&body_family)
                    .foreground("#7f848e")
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
        .halign(gtk::Align::Center)
        .margin_start(16)
        .margin_end(16)
        .margin_bottom(16)
        .css_classes(["pill", "suggested-action"])
        .build();

    let audio_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    audio_box.append(&audio_art_stack);
    audio_box.append(&audio_status);
    audio_box.append(&audio_btn);
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
    stack.add_named(&text_row, Some("text"));

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
        .build();
    stack.add_named(&pdf_scroller, Some("pdf"));

    let table_view = gtk::ColumnView::builder()
        .show_row_separators(true)
        .show_column_separators(false)
        .build();
    let table_scroller = gtk::ScrolledWindow::builder()
        .child(&table_view)
        .vexpand(true)
        .hexpand(true)
        .build();
    stack.add_named(&table_scroller, Some("table"));

    let meta_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .valign(gtk::Align::Start)
        .build();
    let meta_scroller = gtk::ScrolledWindow::builder()
        .child(&meta_list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    stack.add_named(&meta_scroller, Some("meta"));

    stack.set_visible_child_name("empty");

    // Placeholder, never inserted anywhere, so calls to `footer.set_visible`
    // in `apply` do not need to special-case the panel variants that skip it.
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let details_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(12)
        .valign(gtk::Align::Start)
        .build();
    let details_scroller = gtk::ScrolledWindow::builder()
        .child(&details_list)
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
        wide_btn.clone().upcast(),
    ];
    {
        let s = sender.clone();
        let header = panel_header(
            Some("view-compact-symbolic"),
            &tr("Preview"),
            &extras,
            move || s.input(AppMsg::TogglePreview),
        );
        panel.append(&header);
    }
    {
        let s = sender.clone();
        wide_btn.connect_toggled(move |b| s.input(AppMsg::SetWidePreviewDefault(b.is_active())));
    }
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
    panel.append(&view_stack);

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

    audio_btn.connect_clicked(|btn| {
        let is_playing = AUDIO_MPV_PID.load(Ordering::SeqCst) != 0;
        if is_playing {
            stop_audio_mpv();
            btn.set_label(&tr("Play"));
            preview_registry::with(|h| {
                h.audio_status.set_label(&tr("Stopped."));
            });
            return;
        }
        let path = preview_registry::with(|h| h.state.path.borrow().clone()).flatten();
        let Some(path) = path else { return };
        match spawn_audio_mpv(&path) {
            Ok(pid) => {
                AUDIO_MPV_PID.store(pid, Ordering::SeqCst);
                btn.set_label(&tr("Stop"));
                preview_registry::with(|h| {
                    h.audio_status.set_label(&tr("Playing…"));
                });
            }
            Err(msg) => {
                eprintln!("[flux][preview] audio: {msg}");
                preview_registry::with(|h| {
                    h.audio_status.set_label(&tr(
                        "Audio preview requires mpv. Install it with your package manager.",
                    ));
                });
                btn.set_sensitive(false);
            }
        }
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

    pub fn apply(&self, content: &PreviewContent) {
        let st = &self.state;
        st.token.set(st.token.get() + 1);
        self.stop_media();
        self.banner.set_revealed(false);

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
                let is_font = crate::services::fonts::is_font_path(&info.path);
                let installed = is_font && crate::services::fonts::is_installed(&info.path);
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

            PreviewContent::Listing { rows, more, info } => {
                self.show_info(info);
                self.set_mode(Mode::None);
                clear_list(&self.meta_list);
                if rows.is_empty() {
                    push_row(&self.meta_list, &tr("Empty"), "");
                }
                for (k, v) in rows {
                    push_row(&self.meta_list, k, v);
                }
                if *more > 0 {
                    push_row(&self.meta_list, "…", &format!("+{more}"));
                }
                self.stack.set_visible_child_name("meta");
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
                    mf.set_file(Some(&gtk::gio::File::for_path(path)));
                    self.media_video.set_media_stream(Some(&mf));
                    *self.state.media.borrow_mut() = Some(mf);
                    self.stack.set_visible_child_name("media-video");
                } else {
                    if AUDIO_MPV_PID.load(Ordering::SeqCst) != 0 {
                        stop_audio_mpv();
                    }
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
                self.audio_btn.set_label(&tr("Stop"));
                self.audio_status.set_label(&tr("Playing…"));
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
            self.audio_btn.set_label(&tr("Play"));
            self.audio_status.set_label(&tr("Stopped."));
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
        clear_list(&self.details_list);
        push_row(&self.details_list, &tr("Name"), &info.name);
        push_row(&self.details_list, &tr("Type"), &info.mime);
        let virtual_path = info
            .path
            .to_string_lossy()
            .starts_with(crate::services::archive::ARCHIVE_URI);
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
        let vw = self.stack.width() - PDF_GUTTER;
        if vw <= 8 {
            return 0;
        }
        ((vw as f64) * self.state.zoom.get()).round().max(32.0) as i32
    }

    fn render_pdf(&self) {
        let token = self.state.token.get() + 1;
        self.state.token.set(token);
        self.render_pdf_inner(token, 0);
    }

    fn render_pdf_inner(&self, token: u64, attempt: u32) {
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
    provider.load_from_data(
        ".flux-pdf-page { box-shadow: 0 1px 6px alpha(black, 0.35); }\n\
         .flux-pdf-invert { filter: invert(1) hue-rotate(180deg); }\n\
         .flux-preview-gutter { background-color: alpha(currentColor, 0.03); }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn flat_icon_button(icon: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tip)
        .css_classes(["flat"])
        .build()
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
        .subtitle_selectable(true)
        .css_classes(["property"])
        .build();
    l.append(&row);
}

/// Byte offsets to char offsets. `GtkTextBuffer::iter_at_offset` counts
/// chars, but `syntect` and `pulldown-cmark` hand back byte ranges.
fn byte_to_char_map(text: &str) -> Vec<i32> {
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
