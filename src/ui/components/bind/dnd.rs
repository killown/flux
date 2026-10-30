//! Drag source payload and drop target actions for a cell.
//!
//! # Contract
//!
//! - [`bind`] builds a `ContentProvider` per bind, which the parent contract
//!   forbids. The contract names a cached `FileItem::content_provider`, but
//!   `FileItem` has no such field today, so the provider is still built here.
//!   Caching it at load time is the open item for this module.
//! - Folders accept COPY | MOVE drops, files accept none.
//! - [`release`] must undo everything [`bind`] set on the drag source so a
//!   recycled cell never drags the previous item.

use crate::ui::components::widgets::FileWidgets;
use crate::ui::components::FileItem;
use adw::gdk;
use adw::prelude::*;
use gtk::glib;

pub(super) fn bind(item: &FileItem, widgets: &FileWidgets) {
    if item.disable_drag_and_drop {
        crate::hit!("bind_dnd_disabled");
        widgets
            .drag_source
            .set_content(None::<&gdk::ContentProvider>);
        widgets.drop_target.set_actions(gdk::DragAction::empty());
    } else {
        crate::hit!("bind_dnd_payload");
        let file = gtk::gio::File::for_path(&item.path);
        let uri = format!("{}\r\n", file.uri());
        let file_list_provider =
            gdk::ContentProvider::for_bytes("text/uri-list", &glib::Bytes::from(uri.as_bytes()));
        let file_provider = gdk::ContentProvider::for_value(&file.to_value());
        let content = gdk::ContentProvider::new_union(&[file_list_provider, file_provider]);
        widgets.drag_source.set_content(Some(&content));

        widgets.drop_target.set_actions(if item.is_dir {
            gdk::DragAction::COPY | gdk::DragAction::MOVE
        } else {
            gdk::DragAction::empty()
        });
    }
}

/// Clears the drag payload and drag icon.
pub(super) fn release(widgets: &FileWidgets) {
    widgets
        .drag_source
        .set_content(None::<&gdk::ContentProvider>);
    widgets.drag_source.set_icon(None::<&gdk::Paintable>, 0, 0);
}
