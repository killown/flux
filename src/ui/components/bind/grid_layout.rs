//! Grid-mode layout: a vertical card, icon on top and label below.
//!
//! # Contract
//!
//! Called from `bind_widgets` on every scroll tick, so the parent contract
//! applies: no syscalls, no filesystem-derived string formatting, no new
//! GObjects.
//!
//! This function must restore every property `list_layout::apply` changes,
//! because cells are recycled between modes. If you add a property to the
//! list layout, add its grid default here.

use crate::ui::components::widgets::FileWidgets;
use crate::ui::components::FileItem;
use crate::ui::constants;
use adw::prelude::*;

/// Lays out `widgets` as a grid card for `item`.
pub(super) fn apply(item: &FileItem, widgets: &mut FileWidgets) {
    crate::hit!("bind_layout_grid");
    widgets.card_box.set_orientation(gtk::Orientation::Vertical);
    widgets.card_box.set_halign(gtk::Align::Fill);
    widgets.card_box.set_valign(gtk::Align::Fill);
    widgets.card_box.set_hexpand(false);
    widgets.card_box.set_vexpand(false);
    widgets.card_box.set_spacing(0);
    widgets.icon_widget.set_pixel_size(item.icon_size);
    widgets.icon_widget.set_size_request(-1, -1);
    widgets.icon_widget.set_valign(gtk::Align::End);
    widgets.icon_widget.set_halign(gtk::Align::Center);
    if let Some(frame) = widgets.preview_stack.child_by_name("video") {
        frame.set_size_request(item.icon_size, item.icon_size);
    }
    widgets.preview_stack.set_size_request(-1, -1);
    widgets.preview_stack.set_valign(gtk::Align::End);
    widgets.preview_stack.set_halign(gtk::Align::Center);
    widgets.preview_stack.set_visible_child_name("icon");
    widgets.label.set_halign(gtk::Align::Center);
    widgets.label.set_hexpand(false);
    widgets.label.set_justify(gtk::Justification::Center);
    widgets.label.set_max_width_chars(item.max_width_chars);
    widgets.label.set_width_chars(item.grid_spacing);

    apply_font_scale(item, widgets);

    if item.expand_labels {
        widgets.label.set_wrap(true);
        widgets.label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::None);
    } else {
        widgets.label.set_wrap(false);
        widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    }

    // Restore scroller to grid-mode defaults: no scrollbar, propagate natural size.
    widgets
        .label_scroller
        .set_hscrollbar_policy(gtk::PolicyType::Never);
    widgets.label_scroller.set_propagate_natural_width(true);
    widgets.label_scroller.set_valign(gtk::Align::Start);
    widgets.label_scroller.set_hexpand(false);

    // Reset Stack alignment for grid mode (centered horizontally, anchored to start vertically).
    widgets.stack.set_halign(gtk::Align::Center);
    widgets.stack.set_valign(gtk::Align::Start);
    widgets.stack.set_hexpand(false);
    if let Some(label_box) = widgets.stack.child_by_name(constants::VIEW_LABEL) {
        if let Some(box_widget) = label_box.downcast_ref::<gtk::Box>() {
            box_widget.set_halign(gtk::Align::Center);
            box_widget.set_valign(gtk::Align::Start);
            box_widget.set_hexpand(false);
            box_widget.set_size_request(-1, -1);
        }
    }

    widgets.info_label.set_visible(false);
}

fn apply_font_scale(item: &FileItem, widgets: &FileWidgets) {
    if item.scale_font_with_icons && item.icon_size > 0 {
        let base_size = if item.default_icon_size > 0 {
            item.default_icon_size as f64
        } else {
            96.0f64
        };
        let scale_factor = (item.icon_size as f64 / base_size).clamp(0.8, 1.3);
        let attrs = gtk::pango::AttrList::new();
        attrs.insert(gtk::pango::AttrFloat::new_scale(scale_factor));
        widgets.label.set_attributes(Some(&attrs));
    } else {
        widgets.label.set_attributes(None);
    }
}
