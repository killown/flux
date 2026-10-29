//! Preview colors must stay legible in both libadwaita color schemes.

use flux::ui::panels::preview::syntax::{highlight, set_dark, tag_colors};

/// WCAG relative luminance of a `#rrggbb` color.
fn luminance(hex: &str) -> f64 {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
    let channel = |shift: u32| {
        let c = ((n >> shift) & 0xff) as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

fn contrast(fg: &str, bg: &str) -> f64 {
    let (a, b) = (luminance(fg), luminance(bg));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// One test, because the active scheme is a process-wide flag.
#[test]
fn palettes_and_syntax_theme_follow_the_color_scheme() {
    // Libadwaita window backgrounds: light #fafafb, dark #242424.
    for (dark, bg) in [(false, "#fafafb"), (true, "#242424")] {
        set_dark(dark);
        let c = tag_colors();
        for (name, fg) in [
            ("added", c.added),
            ("removed", c.removed),
            ("header", c.header),
            ("code_block", c.code_block),
            ("link", c.link),
            ("quote", c.quote),
        ] {
            let ratio = contrast(fg, bg);
            assert!(
                ratio >= 3.0,
                "{name} {fg} on {bg} has contrast {ratio:.2} (dark={dark})"
            );
        }
        assert!(
            contrast(c.code_fg, c.code_bg) >= 3.0,
            "inline code (dark={dark})"
        );
    }

    let src = "fn main() { let x = \"hi\"; }\n";
    set_dark(true);
    let (dark_ranges, lang) = highlight(src, "rs");
    assert_eq!(lang.as_deref(), Some("Rust"));
    set_dark(false);
    let (light_ranges, _) = highlight(src, "rs");

    assert!(!dark_ranges.is_empty() && !light_ranges.is_empty());
    let colors = |r: &[flux::ui::panels::preview::syntax::HighlightedRange]| {
        r.iter().map(|h| h.fg).collect::<Vec<_>>()
    };
    assert_ne!(colors(&dark_ranges), colors(&light_ranges));
}
