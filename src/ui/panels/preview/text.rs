//! Text and Markdown viewer support: the syntect and Markdown tag tables, and the
//! byte to char offset map that `GtkTextBuffer` needs.

use super::handle::PreviewPanelHandle;
use super::syntax;
use crate::domain::model::{HighlightedRange, MdStyle, StyledRange};
use adw::prelude::*;
use std::collections::HashMap;
use std::ops::Range;

/// Style key for deduplicating syntax-highlight text tags:
/// (foreground RGB, bold, italic, underline).
type SyntaxStyleKey = ((u8, u8, u8), bool, bool, bool);

pub(super) fn body_font_family() -> String {
    gtk::Settings::default()
        .and_then(|s| s.gtk_font_name())
        .and_then(|n| {
            pango::FontDescription::from_string(&n)
                .family()
                .map(|f| f.to_string())
        })
        .unwrap_or_else(|| "sans-serif".to_string())
}

/// Byte offsets to char offsets. `GtkTextBuffer::iter_at_offset` counts
/// chars, but `syntect` and `pulldown-cmark` hand back byte ranges.
pub(super) fn byte_to_char_map(text: &str) -> Vec<i32> {
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

impl PreviewPanelHandle {
    pub(super) fn rebuild_syntax_tags(
        &self,
        ranges: &[HighlightedRange],
    ) -> Vec<(String, Range<usize>)> {
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
        let colors = syntax::tag_colors();

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

    pub(super) fn apply_markdown_view(&self) {
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
}
