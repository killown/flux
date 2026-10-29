//! Small widget constructors and list helpers used while building the panel.

use adw::prelude::*;
use std::cell::Cell;

const INFO_MAX_WIDTH: i32 = 720;

pub(super) fn install_css() {
    crate::hit!("install_css");
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
pub(super) fn first_available_icon(names: &[&str]) -> String {
    let theme = gtk::gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d));
    names
        .iter()
        .find(|n| theme.as_ref().is_some_and(|t| t.has_icon(n)))
        .or(names.last())
        .map(|n| n.to_string())
        .unwrap_or_default()
}

pub(super) fn flat_icon_button(icon: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tip)
        .css_classes(["flat"])
        .build()
}

/// Caps an info list's width and centres it so wide panels keep it clear of the nav arrows.
pub(super) fn info_clamp(child: &impl IsA<gtk::Widget>) -> adw::Clamp {
    adw::Clamp::builder()
        .maximum_size(INFO_MAX_WIDTH)
        .tightening_threshold(INFO_MAX_WIDTH)
        .child(child)
        .build()
}

pub(super) fn clear_list(l: &gtk::ListBox) {
    while let Some(c) = l.first_child() {
        l.remove(&c);
    }
}

pub(super) fn push_row(l: &gtk::ListBox, key: &str, value: &str) {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(key))
        .subtitle(glib::markup_escape_text(value))
        .build();
    l.append(&row);
}
