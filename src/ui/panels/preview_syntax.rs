//! Syntax highlighting helper for the peek-preview text viewer.
//!
//! Runs inside `compute_preview` on the blocking worker so the (one-time)
//! `SyntaxSet` construction never touches the GTK main thread. The widget
//! side only consumes `Vec<HighlightedRange>` - plain byte offsets - and
//! turns them into `gtk::TextTag`s.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Byte range inside the joined preview text that shares one visual style.
#[derive(Debug, Clone, Copy)]
pub struct HighlightedRange {
    pub start: usize,
    pub end: usize,
    pub fg: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

static DARK: AtomicBool = AtomicBool::new(true);

/// Records the active color scheme, safe to read from the preview worker thread.
pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

/// Returns whether the app is currently using the dark color scheme.
pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

/// Foreground colors for markdown and diff tags, tuned for the active scheme.
pub struct TagColors {
    pub added: &'static str,
    pub removed: &'static str,
    pub header: &'static str,
    pub code_fg: &'static str,
    pub code_bg: &'static str,
    pub code_block: &'static str,
    pub link: &'static str,
    pub quote: &'static str,
}

/// Returns the tag palette for the active color scheme.
pub fn tag_colors() -> TagColors {
    crate::hit!("tag_colors");
    if is_dark() {
        TagColors {
            added: "#57ab5a",
            removed: "#e5534b",
            header: "#76e3ea",
            code_fg: "#e5c07b",
            code_bg: "#2a2a2a",
            code_block: "#98c379",
            link: "#61afef",
            quote: "#7f848e",
        }
    } else {
        TagColors {
            added: "#1a7f37",
            removed: "#cf222e",
            header: "#0969da",
            code_fg: "#953800",
            code_bg: "#eff1f3",
            code_block: "#116329",
            link: "#0969da",
            quote: "#57606a",
        }
    }
}

fn theme(dark: bool) -> &'static Theme {
    crate::hit!("theme");
    static TS: OnceLock<ThemeSet> = OnceLock::new();
    let ts = TS.get_or_init(ThemeSet::load_defaults);
    let name = if dark {
        "base16-ocean.dark"
    } else {
        "InspiredGitHub"
    };
    ts.themes
        .get(name)
        .or_else(|| ts.themes.values().next())
        .expect("syntect default theme set is non-empty")
}

/// Returns `(ranges, language_name)`. `language_name` is `None` when syntect
/// could not match the extension - the caller should not install tags in
/// that case and can still display the raw text.
pub fn highlight(text: &str, ext: &str) -> (Vec<HighlightedRange>, Option<String>) {
    crate::hit!("highlight");
    let ss = syntax_set();
    let Some(syntax) = ss.find_syntax_by_extension(ext) else {
        return (Vec::new(), None);
    };
    let lang = syntax.name.clone();

    let mut h = HighlightLines::new(syntax, theme(is_dark()));
    let mut ranges = Vec::new();
    let mut offset = 0usize;

    for line in LinesWithEndings::from(text) {
        let line_start = offset;
        let Ok(spans) = h.highlight_line(line, ss) else {
            // A single malformed line must not kill the whole preview.
            return (Vec::new(), Some(lang));
        };
        for (style, chunk) in spans {
            let start = offset;
            let end = offset + chunk.len();
            offset = end;
            if end > start {
                ranges.push(range_from(style, start, end));
            }
        }
        // `LinesWithEndings` includes the terminator, so offset is already
        // past it, nothing else to do.
        debug_assert!(offset >= line_start);
    }

    (ranges, Some(lang))
}

fn range_from(style: Style, start: usize, end: usize) -> HighlightedRange {
    let f = style.foreground;
    HighlightedRange {
        start,
        end,
        fg: (f.r, f.g, f.b),
        bold: style
            .font_style
            .contains(syntect::highlighting::FontStyle::BOLD),
        italic: style
            .font_style
            .contains(syntect::highlighting::FontStyle::ITALIC),
        underline: style
            .font_style
            .contains(syntect::highlighting::FontStyle::UNDERLINE),
    }
}
