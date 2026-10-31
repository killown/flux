//! Pure input helpers: pointer-button parsing and the preview-key predicates.
//!
//! These take only GDK values, so they live below `app` and are called from the
//! update layer, the view code and the fuzz targets.

use gtk::gdk;

/// Parses a string representation of a pointer button into its corresponding numeric identifier.
///
/// Maps standard identifiers (such as button/pointer names or back/forward aliases)
/// to their numerical button values (e.g., 8 for Back and 9 for Forward).
pub fn parse_mouse_button(raw: &str) -> Option<u32> {
    let s = raw.trim().to_lowercase();
    match s.as_str() {
        "button8" | "pointer8" | "mouse8" | "back" => Some(8),
        "button9" | "pointer9" | "mouse9" | "forward" => Some(9),
        "button2" | "pointer2" | "mouse2" | "middle" => Some(2),
        "button1" | "pointer1" | "mouse1" | "left" => Some(1),
        "button3" | "pointer3" | "mouse3" | "right" => Some(3),
        _ => None,
    }
}

// ── PEEK PREVIEW ──────────────────────────────────────────────────────────
/// Predicate for whether `Ctrl+Space` should toggle the peek-preview panel.
pub fn should_trigger_preview(
    keyval: gdk::Key,
    modifiers: gdk::ModifierType,
    is_editable: bool,
    terminal_focused: bool,
) -> bool {
    let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
    keyval == gdk::Key::space && ctrl && !is_editable && !terminal_focused
}

/// Predicate for whether bare `Space` should open the preview panel and
/// apply the selected item's MIME filter. Blocked in text entries and in the
/// embedded terminal, where Space has its own meaning.
pub fn should_trigger_preview_and_mime(
    keyval: gdk::Key,
    modifiers: gdk::ModifierType,
    is_editable: bool,
    terminal_focused: bool,
) -> bool {
    keyval == gdk::Key::space && modifiers.is_empty() && !is_editable && !terminal_focused
}
// ──────────────────────────────────────────────────────────────────────────
