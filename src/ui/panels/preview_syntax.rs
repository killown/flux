//! Syntax highlighting helper for the peek-preview text viewer.
//!
//! Runs inside `compute_preview` on the blocking worker so the (one-time)
//! `SyntaxSet` construction never touches the GTK main thread. The widget
//! side only consumes `Vec<HighlightedRange>` - plain byte offsets - and
//! turns them into `gtk::TextTag`s.

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

fn theme() -> &'static Theme {
    static TH: OnceLock<Theme> = OnceLock::new();
    TH.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        ts.themes
            .get("base16-ocean.dark")
            .cloned()
            .or_else(|| ts.themes.values().next().cloned())
            .expect("syntect default theme set is non-empty")
    })
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

    let mut h = HighlightLines::new(syntax, theme());
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
