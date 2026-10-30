//! List-mode layout: a compact horizontal row.
//!
//! Small icon on the left, filename filling the rest, pre-formatted info
//! label on the right.
//!
//! # Contract
//!
//! Called from `bind_widgets` on every scroll tick, so the parent contract
//! applies: no syscalls, no filesystem-derived string formatting, no new
//! GObjects. The only allocations allowed here are the tiny `AttrList`s for
//! font scaling and the `format!` for content-search snippets.

use super::remove_style_provider;
use crate::ui::components::widgets::FileWidgets;
use crate::ui::components::FileItem;
use crate::ui::constants;
use adw::prelude::*;

/// Lays out `widgets` as a list row for `item`.
pub(super) fn apply(item: &FileItem, widgets: &mut FileWidgets, root: &gtk::Overlay) {
    crate::hit!("bind_layout_list");
    // Compact horizontal row: small icon on the left, filename fills the rest.
    widgets
        .card_box
        .set_orientation(gtk::Orientation::Horizontal);
    widgets.card_box.set_halign(gtk::Align::Fill);
    widgets.card_box.set_valign(gtk::Align::Center);
    widgets.card_box.set_hexpand(true);
    widgets.card_box.set_vexpand(false);
    widgets.card_box.set_spacing(10);

    widgets.icon_widget.set_pixel_size(item.icon_size);
    widgets
        .icon_widget
        .set_size_request(item.icon_size, item.icon_size);
    widgets.icon_widget.set_valign(gtk::Align::Center);
    widgets.icon_widget.set_halign(gtk::Align::Start);

    if let Some(frame) = widgets.preview_stack.child_by_name("video") {
        frame.set_size_request(item.icon_size, item.icon_size);
    }

    widgets
        .preview_stack
        .set_size_request(item.icon_size, item.icon_size);
    widgets.preview_stack.set_valign(gtk::Align::Center);
    widgets.preview_stack.set_halign(gtk::Align::Start);
    widgets.preview_stack.set_visible_child_name("icon");
    widgets.label.set_halign(gtk::Align::Start);
    widgets.label.set_hexpand(true);
    widgets.label.set_justify(gtk::Justification::Left);
    widgets.label.set_width_chars(1);
    widgets
        .label
        .set_max_width_chars(item.max_width_chars.max(10));
    widgets.label.set_wrap(false);
    widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    if let Some(ref old_provider) = widgets.scale_css_provider {
        remove_style_provider(&widgets.label, old_provider);
        remove_style_provider(&widgets.info_label, old_provider);
        widgets.scale_css_provider = None;
    }

    apply_font_scale(item, widgets);

    // In list mode the scroller provides horizontal scrolling so the row
    // never grows wider than its allocated column width.
    widgets
        .label_scroller
        .set_hscrollbar_policy(gtk::PolicyType::Automatic);
    widgets.label_scroller.set_propagate_natural_width(false);
    widgets.label_scroller.set_propagate_natural_height(true);
    widgets.label_scroller.set_valign(gtk::Align::Center);
    widgets.label_scroller.set_hexpand(true);

    // Stack and its label-box fill the scroller's viewport.
    widgets.stack.set_halign(gtk::Align::Fill);
    widgets.stack.set_valign(gtk::Align::Center);
    widgets.stack.set_hexpand(true);

    // Get the label container (the Box inside the Stack) and make it start-aligned.
    if let Some(label_box) = widgets.stack.child_by_name(constants::VIEW_LABEL) {
        if let Some(box_widget) = label_box.downcast_ref::<gtk::Box>() {
            box_widget.set_halign(gtk::Align::Start);
            box_widget.set_valign(gtk::Align::Center);
            box_widget.set_hexpand(true);
            box_widget.set_size_request(-1, -1);
        }
    }

    // Invalidate cached measurements across the scroller boundary to force size renegotiation
    widgets.label.queue_resize();
    widgets.info_label.queue_resize();
    widgets.label_scroller.queue_resize();
    root.queue_resize();

    bind_info_label(item, widgets);
}

/// Scales label and info label with the icon size when the user asked for it.
fn apply_font_scale(item: &FileItem, widgets: &FileWidgets) {
    if item.scale_font_with_icons && item.icon_size > 0 {
        const DEFAULT_LIST_ICON_BASELINE: f64 = 64.0;

        let ratio = item.icon_size as f64 / DEFAULT_LIST_ICON_BASELINE;

        // Allow a broader clamp so size differences between 16px and 48px+ are visible
        let scale_factor = ratio.clamp(0.8, 2.0);

        let label_attrs = gtk::pango::AttrList::new();
        label_attrs.insert(gtk::pango::AttrFloat::new_scale(scale_factor));
        widgets.label.set_attributes(Some(&label_attrs));

        let info_attrs = gtk::pango::AttrList::new();
        info_attrs.insert(gtk::pango::AttrFloat::new_scale(scale_factor));
        widgets.info_label.set_attributes(Some(&info_attrs));
    } else {
        widgets.label.set_attributes(None);
        widgets.info_label.set_attributes(None);
    }
}

/// Info label: pre-formatted, zero alloc, zero syscall.
///
/// `item.labels` was computed once by `FileLoadContext::compute_labels`
/// in the Rayon pool. Nothing here formats a date, formats a size, or
/// calls `glib::DateTime::now_local()`.
fn bind_info_label(item: &FileItem, widgets: &FileWidgets) {
    crate::hit!("bind_info_label");
    widgets.info_label.set_halign(gtk::Align::End);
    widgets.info_label.set_xalign(1.0);
    widgets.info_label.set_hexpand(false);

    if let Some(ref snippet) = item.search_snippet {
        // Search snippet is per-item and can't be pre-formatted
        // (content depends on the match offset), so it stays here.
        // `format!` here is cheap and rare (only content-search results).
        if item.line_number > 0 {
            widgets
                .info_label
                .set_label(&format!("L:{}  {}", item.line_number, snippet));
        } else {
            widgets.info_label.set_label(snippet);
        }
        widgets.info_label.set_visible(true);
        widgets.info_label.set_tooltip_text(None::<&str>);
    } else if item.labels.has_info() {
        widgets.info_label.set_label(&item.labels.info_combined);
        if !item.labels.mtime_tooltip.is_empty() {
            widgets
                .info_label
                .set_tooltip_text(Some(&item.labels.mtime_tooltip));
        } else {
            widgets.info_label.set_tooltip_text(None::<&str>);
        }
        widgets.info_label.set_visible(true);
    } else {
        widgets.info_label.set_tooltip_text(None::<&str>);
        widgets.info_label.set_visible(false);
    }
}
