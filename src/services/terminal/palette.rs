//! ANSI colour palette and colour conversion helpers.

/// Maps a 3-bit ANSI color index (0-7) to an RGBA value.
/// `bright` selects the high-intensity variant used by SGR 90-97 / 100-107.
#[inline]
/// Returns the xterm-compatible 16-color ANSI palette as a fallback.
///
/// Indices 0-7 are the normal colors, 8-15 are their bright counterparts.
/// [`TerminalState::apply_theme`] overwrites individual slots with GTK
/// theme values at runtime, so these are only used before the first theme
/// application or for palette entries with no matching GTK variable.
pub(super) fn default_ansi_palette() -> [gtk::gdk::RGBA; 16] {
    const ENTRIES: [(f32, f32, f32); 16] = [
        (0.0, 0.0, 0.0), // 0  black
        (0.8, 0.0, 0.0), // 1  red
        (0.0, 0.8, 0.0), // 2  green
        (0.8, 0.8, 0.0), // 3  yellow
        (0.0, 0.0, 0.8), // 4  blue
        (0.8, 0.0, 0.8), // 5  magenta
        (0.0, 0.8, 0.8), // 6  cyan
        (0.8, 0.8, 0.8), // 7  white
        (0.4, 0.4, 0.4), // 8  bright black
        (1.0, 0.2, 0.2), // 9  bright red
        (0.2, 1.0, 0.2), // 10 bright green
        (1.0, 1.0, 0.2), // 11 bright yellow
        (0.2, 0.2, 1.0), // 12 bright blue
        (1.0, 0.2, 1.0), // 13 bright magenta
        (0.2, 1.0, 1.0), // 14 bright cyan
        (1.0, 1.0, 1.0), // 15 bright white
    ];
    ENTRIES.map(|(r, g, b)| gtk::gdk::RGBA::new(r, g, b, 1.0))
}

/// Retained for use in [`TerminalState::color_from_256`] (indices 0-15 of the
/// 256-color palette still need a static lookup path).
#[allow(dead_code)]
fn ansi_color(index: u16, bright: bool) -> gtk::gdk::RGBA {
    let palette = default_ansi_palette();
    let idx = if bright {
        8 + (index as usize).min(7)
    } else {
        (index as usize).min(7)
    };
    palette[idx]
}

/// Converts a 24-bit RGB triple (0-255 each, passed as i64) into RGBA.
#[inline]
pub(super) fn rgb_color(r: i64, g: i64, b: i64) -> gtk::gdk::RGBA {
    gtk::gdk::RGBA::new(
        (r as f32 / 255.0).clamp(0.0, 1.0),
        (g as f32 / 255.0).clamp(0.0, 1.0),
        (b as f32 / 255.0).clamp(0.0, 1.0),
        1.0,
    )
}
