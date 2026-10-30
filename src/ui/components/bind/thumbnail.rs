//! Thumbnail / icon painting and lazy-thumbnail hook events.
//!
//! # Contract
//!
//! - [`paint`] only paints what is already resolved: the cached thumbnail
//!   `Texture`, or whatever `gicon::paintable_for_gicon` returns (cache hit or
//!   theme fallback). It never decodes.
//! - [`report_bound`] and [`report_unbound`] are the only places that talk to
//!   a [`ThumbHook`](crate::ui::components::ThumbHook). The hook is called
//!   synchronously on the GTK thread, so the callback must be cheap.
//! - A bound event is sent only for cells showing a plain icon
//!   (`thumbnail.is_none() && !is_dir`).

use super::gicon;
use crate::ui::components::widgets::FileWidgets;
use crate::ui::components::{FileItem, ThumbEvent};

/// Paints the thumbnail if there is one, the item's icon otherwise.
pub(super) fn paint(item: &FileItem, widgets: &FileWidgets, root: &gtk::Overlay) {
    if let Some(ref texture) = item.thumbnail {
        crate::hit!("bind_thumbnail_paint");
        if item.thumb_hook.is_some() {
            // A freshly created cell has no pixel size yet, and a thumbnail would
            // paint at its natural size (up to 256px) instead of the grid's icon size.
            widgets.icon_widget.set_pixel_size(item.icon_size);
        }
        widgets.icon_widget.set_paintable(Some(texture));
    } else {
        crate::hit!("bind_icon_paint");
        widgets.icon_widget.set_pixel_size(item.icon_size);

        let paintable = gicon::paintable_for_gicon(item, root, &item.icon);
        widgets.icon_widget.set_paintable(Some(&paintable));
    }
}

/// Lazy thumbnails (folder grid): report cells that are showing a plain icon.
pub(super) fn report_bound(item: &FileItem, widgets: &FileWidgets) {
    if let Some(hook) = &item.thumb_hook {
        if item.thumbnail.is_none() && !item.is_dir {
            (hook.0)(ThumbEvent::Bound {
                idx: item.grid_idx,
                path: item.path.clone(),
                icon: widgets.icon_widget.clone(),
            });
        }
    }
}

/// Tells the hook this cell no longer shows `item`, so the next item bound to
/// the recycled cell is not suppressed.
pub(super) fn report_unbound(item: &FileItem) {
    if let Some(hook) = &item.thumb_hook {
        (hook.0)(ThumbEvent::Unbound { idx: item.grid_idx });
    }
}
