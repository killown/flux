//! Markdown renderer for the peek-preview text viewer.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdStyle {
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,
    Bold,
    Italic,
    Code,
    CodeBlock,
    Link,
    BlockQuote,
    ListBullet,
}

#[derive(Debug, Clone)]
pub struct StyledRange {
    pub range: Range<usize>,
    pub style: MdStyle,
}

#[derive(Debug, Clone, Default)]
pub struct MarkdownRender {
    pub text: String,
    pub styles: Vec<StyledRange>,
}

/// Renders `source` as plain text with style ranges.
pub fn render(source: &str) -> MarkdownRender {
    crate::hit!("render");
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(source, opts);

    let mut out = MarkdownRender::default();
    let mut style_stack: Vec<MdStyle> = Vec::new();
    let mut range_start: usize = 0;

    for ev in parser {
        match ev {
            Event::Start(tag) => {
                range_start = out.text.len();
                if let Some(style) = style_for_start(&tag) {
                    style_stack.push(style);
                }
                // Prepend a list bullet or blockquote marker so the flat
                // text still reads as a list when tags are ignored.
                match &tag {
                    Tag::Item => out.text.push_str("• "),
                    Tag::BlockQuote(_) => out.text.push_str("> "),
                    Tag::CodeBlock(_) => out.text.push_str("```\n"),
                    _ => {}
                }
            }
            Event::End(tag) => {
                let end = out.text.len();
                if style_for_end(&tag) {
                    if let Some(style) = style_stack.pop() {
                        if end > range_start {
                            out.styles.push(StyledRange {
                                range: range_start..end,
                                style,
                            });
                        }
                    }
                }
                if matches!(tag, TagEnd::CodeBlock) {
                    out.text.push_str("\n```\n");
                }
                if matches!(tag, TagEnd::Paragraph | TagEnd::Heading(_)) {
                    out.text.push('\n');
                    out.text.push('\n');
                }
            }
            Event::Text(t) => out.text.push_str(&t),
            Event::Code(t) => {
                let start = out.text.len();
                out.text.push_str(&t);
                let end = out.text.len();
                if end > start {
                    out.styles.push(StyledRange {
                        range: start..end,
                        style: MdStyle::Code,
                    });
                }
            }
            Event::SoftBreak => out.text.push('\n'),
            Event::HardBreak => {
                out.text.push('\n');
            }
            Event::Rule => {
                out.text.push_str("\n────────────────────\n");
            }
            Event::TaskListMarker(done) => {
                out.text.push_str(if done { "[x] " } else { "[ ] " });
            }
            _ => {}
        }
    }

    out
}

fn style_for_start(tag: &Tag) -> Option<MdStyle> {
    Some(match tag {
        Tag::Heading { level, .. } => match level {
            HeadingLevel::H1 => MdStyle::Heading1,
            HeadingLevel::H2 => MdStyle::Heading2,
            HeadingLevel::H3 => MdStyle::Heading3,
            HeadingLevel::H4 => MdStyle::Heading4,
            HeadingLevel::H5 => MdStyle::Heading5,
            HeadingLevel::H6 => MdStyle::Heading6,
        },
        Tag::Emphasis => MdStyle::Italic,
        Tag::Strong => MdStyle::Bold,
        Tag::CodeBlock(_) => MdStyle::CodeBlock,
        Tag::Link { .. } => MdStyle::Link,
        Tag::BlockQuote(_) => MdStyle::BlockQuote,
        Tag::Item => MdStyle::ListBullet,
        _ => return None,
    })
}

fn style_for_end(tag: &TagEnd) -> bool {
    matches!(
        tag,
        TagEnd::Heading(_)
            | TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::CodeBlock
            | TagEnd::Link
            | TagEnd::BlockQuote(_)
            | TagEnd::Item
    )
}
