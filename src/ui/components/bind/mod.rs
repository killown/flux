//! Binds a [`FileItem`] to a recycled grid cell.
//!
//! This module is the orchestrator. It owns the hot-path contract (see
//! [`FileItem::bind_widgets`]) and the small, cheap concerns that do not
//! deserve a file of their own: state CSS classes, emblem classes, the git
//! badge, archive/lock badge visibility and the per-widget path data.
//! Everything else lives in a sibling module with its own contract:
//!
//! | module          | owns                                                  |
//! |-----------------|-------------------------------------------------------|
//! | `list_layout`   | compact row geometry, info label, list font scaling   |
//! | `grid_layout`   | vertical card geometry, grid font scaling             |
//! | `rename_entry`  | inline rename `gtk::Entry` lifecycle                  |
//! | `thumbnail`     | thumbnail / icon paint, lazy-thumbnail hook events    |
//! | `gicon`         | `gio::Icon` -> `Paintable`, off-thread custom decode  |
//! | `dnd`           | drag payload and drop-target actions                  |

mod dnd;
mod gicon;
mod grid_layout;
mod list_layout;
mod rename_entry;
mod thumbnail;

use super::file_item::FileItem;
use super::widgets::FileWidgets;
use adw::prelude::*;
use gtk::glib;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

const CLASS_CUT: &str = "flux-card--cut";
const CLASS_COPY: &str = "flux-card--copy";
const CLASS_SYMLINK: &str = "flux-card--symlink";
const CLASS_BROKEN_SYMLINK: &str = "flux-card--broken-symlink";
const CLASS_EMPTY: &str = "flux-card--empty";

/// Detaches a per-widget CSS provider.
///
/// GTK 4.10 deprecated `GtkStyleContext` without a per-widget replacement, so the
/// deprecation is silenced here, in one place, instead of at every call site.
/// Shared with `list_layout`.
#[allow(deprecated)]
fn remove_style_provider(widget: &impl IsA<gtk::Widget>, provider: &gtk::CssProvider) {
    widget.style_context().remove_provider(provider);
}

/// Adds or removes `class` on both the cell root and the card box.
#[inline]
fn set_class(root: &gtk::Overlay, card_box: &gtk::Box, class: &str, on: bool) {
    if on {
        root.add_css_class(class);
        card_box.add_css_class(class);
    } else {
        root.remove_css_class(class);
        card_box.remove_css_class(class);
    }
}

impl FileItem {
    /// Updates the item's widgets with current data from the [FileItem] model.
    ///
    /// Synchronizes labels, icons, thumbnails, and visibility states (e.g., rename entry).
    ///
    /// # Contract
    ///
    /// `bind_widgets` runs on the GTK main thread, once per visible cell, on
    /// every scroll tick. It **MUST NOT**:
    ///
    /// - perform syscalls (`stat`, `read_dir`, `localtime_r`)
    /// - allocate strings from filesystem data (`format_size`, `%x` format)
    /// - construct new GObjects (`ContentProvider`, `Paintable`, `IconTheme`)
    ///
    /// All of that is done in the Rayon pool at load time:
    ///
    /// - `FileLoadContext::labels()`   - size / date strings, tooltip
    /// - `FileItem::resolved_icon`     - cached `Paintable`
    /// - `FileItem::content_provider`  - cached DnD `ContentProvider`
    ///
    /// Reviewers: if you add code that violates this contract, wrap it in
    /// `crate::hit!()` and confirm with `FLUX_HWGA_OUT=1 flux-fm <dir>` that
    /// `bind::*` totals stay sub-µs.
    pub(super) fn bind_widgets(&mut self, widgets: &mut FileWidgets, root: &mut gtk::Overlay) {
        crate::hit!("bind");
        widgets.label.set_label(&self.display_label);

        set_class(root, &widgets.card_box, CLASS_CUT, self.is_cut);
        set_class(root, &widgets.card_box, CLASS_COPY, self.is_copy);

        if self.is_list_mode {
            list_layout::apply(self, widgets, root);
        } else {
            grid_layout::apply(self, widgets);
        }

        set_flux_path_data(self, widgets, root);
        bind_emblems(self, widgets, root);
        bind_git_badge(self, widgets);

        rename_entry::sync(self, widgets);

        thumbnail::paint(self, widgets, root);
        bind_archive_badges(self, widgets);
        thumbnail::report_bound(self, widgets);

        dnd::bind(self, widgets);

        publish_active_path(self, root);
    }

    /// Clears the per-cell lazy-thumbnail guard so the next item bound to this
    /// recycled widget cell can request its own thumbnail without being suppressed.
    pub(super) fn unbind_widgets(&mut self, widgets: &mut FileWidgets, root: &mut gtk::Overlay) {
        crate::hit!("unbind");
        thumbnail::report_unbound(self);
        release_video_stream(widgets);

        if let Some(ref provider) = widgets.scale_css_provider {
            remove_style_provider(&widgets.label, provider);
            remove_style_provider(&widgets.info_label, provider);
            widgets.scale_css_provider = None;
        }

        clear_state_classes(widgets, root);

        widgets.preview_stack.set_visible_child_name("icon");

        widgets.git_badge.set_visible(false);
        widgets.git_badge.set_css_classes(&["flux-git-badge"]);

        // Virtual-dir state must not leak into the next item bound to this cell.
        widgets.icon_overlay.remove_css_class("flux-vd");
        widgets
            .original_widget
            .set_paintable(None::<&gtk::gdk::Paintable>);
        widgets.original_widget.clear();
        widgets.archive_badge.set_visible(false);
        widgets.lock_badge_closed.set_visible(false);
        widgets.lock_badge_open.set_visible(false);

        widgets
            .icon_widget
            .set_paintable(None::<&gtk::gdk::Paintable>);
        widgets.icon_widget.clear();

        dnd::release(widgets);
        widgets.label.set_text("");
        widgets.label.set_attributes(None);
        widgets.info_label.set_text("");
        widgets.info_label.set_attributes(None);
        widgets.info_label.set_tooltip_text(None::<&str>);
        widgets.info_label.set_visible(false);

        rename_entry::remove(widgets);
        clear_path_data(self, widgets, root);
    }
}

/// Stores the item path as opaque widget data on the root and card box.
///
/// WARNING: never use `set_widget_name(&self.path.to_string_lossy())` here.
/// `bind()` runs on every visible item on each scroll/zoom/layout pass.
/// Setting the widget name dirties the CSS node and forces GTK's style
/// engine to re-run a full CSS selector match for the entire grid on
/// every bind, which caused severe scrollbar lag in GridView mode with
/// thousands of items. Opaque widget data via `set_data` bypasses the
/// CSS engine entirely and is the correct, high-performance approach.
fn set_flux_path_data(item: &FileItem, widgets: &FileWidgets, root: &gtk::Overlay) {
    crate::hit!("bind_flux_path_data");
    unsafe {
        root.set_data("flux_path", item.path.clone());
        widgets.card_box.set_data("flux_path", item.path.clone());
    }
}

/// Symlink / broken-symlink / empty-dir emblem classes and the foreign-owner lock.
fn bind_emblems(item: &FileItem, widgets: &FileWidgets, root: &gtk::Overlay) {
    crate::hit!("bind_emblem_classes");
    // Restricted items are marked by the lock badge only.
    widgets.lock_icon.set_visible(item.is_foreign_owner);

    let show_indicator = item.show_symlink_emblem && item.is_symlink;
    set_class(
        root,
        &widgets.card_box,
        CLASS_BROKEN_SYMLINK,
        show_indicator && item.is_broken_symlink,
    );
    set_class(
        root,
        &widgets.card_box,
        CLASS_SYMLINK,
        show_indicator && !item.is_broken_symlink,
    );
    set_class(
        root,
        &widgets.card_box,
        CLASS_EMPTY,
        item.is_dir && item.show_empty_dir_emblem && item.is_empty,
    );
}

fn bind_git_badge(item: &FileItem, widgets: &FileWidgets) {
    crate::hit!("bind_git_badge");
    if let Some((emblem, class_name)) = item.git_status.badge_info() {
        widgets.git_badge.set_label(emblem);
        widgets
            .git_badge
            .set_css_classes(&["flux-git-badge", class_name]);
        widgets.git_badge.set_visible(true);
    } else {
        widgets.git_badge.set_visible(false);
    }
}

/// Archive and encrypted-archive badges.
///
/// Archive drawn as a folder: static badge.
///
/// Encrypted archive padlock is a hint only. The two booleans were resolved by
/// the loader pool (ZIP header read, session sets), so this is visibility
/// toggles: no I/O, no keyring, no allocation, no icon-name or CSS-class
/// churn. Closed padlock = encrypted and locked, open padlock = encrypted and
/// a password is cached.
fn bind_archive_badges(item: &FileItem, widgets: &FileWidgets) {
    widgets.archive_badge.set_visible(item.is_virtual_dir);
    widgets
        .lock_badge_closed
        .set_visible(item.is_encrypted_archive && !item.has_cached_password);
    widgets
        .lock_badge_open
        .set_visible(item.is_encrypted_archive && item.has_cached_password);
}

/// Publishes the current path so gestures can read it.
///
/// The shared cell is read by the right-click gesture, a weak reference to it
/// is stored on the root so gestures reach it without owning the item.
fn publish_active_path(item: &FileItem, root: &gtk::Overlay) {
    crate::hit!("bind_path_cells");
    *item.active_path.borrow_mut() = Some(item.path.clone());
    unsafe {
        root.set_data("active_path_cell", Rc::downgrade(&item.active_path));
        root.set_data("grid_item_index", item.grid_idx);
    }
}

/// Mirror of [`set_flux_path_data`] and [`publish_active_path`].
fn clear_path_data(item: &FileItem, widgets: &FileWidgets, root: &gtk::Overlay) {
    *item.active_path.borrow_mut() = None;
    unsafe {
        let _ = root.steal_data::<Weak<RefCell<Option<PathBuf>>>>("active_path_cell");
        let _ = root.steal_data::<u32>("lazy_thumb_requested");
        let _ = root.steal_data::<u32>("grid_item_index");
        let _ = root.steal_data::<PathBuf>("flux_path");
        let _ = widgets.card_box.steal_data::<PathBuf>("flux_path");
    }
}

fn clear_state_classes(widgets: &FileWidgets, root: &gtk::Overlay) {
    for class in [
        CLASS_CUT,
        CLASS_COPY,
        CLASS_SYMLINK,
        CLASS_BROKEN_SYMLINK,
        CLASS_EMPTY,
    ] {
        set_class(root, &widgets.card_box, class, false);
    }
}

/// Pauses the cell's video and detaches the stream shortly after, unless a
/// newer stream was attached in the meantime.
fn release_video_stream(widgets: &FileWidgets) {
    if let Some(stream) = widgets.video_widget.media_stream() {
        stream.pause();

        let video_weak = widgets.video_widget.downgrade();
        let stream_to_clear = stream.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
            if let Some(video) = video_weak.upgrade() {
                if video
                    .media_stream()
                    .map(|s| s == stream_to_clear)
                    .unwrap_or(false)
                {
                    video.set_media_stream(None::<&gtk::MediaStream>);
                }
            }
        });
    }
}
