//! Inline rename entry lifecycle for a cell.
//!
//! The `gtk::Entry` is created lazily the first time a cell enters edit mode
//! and destroyed again when the cell leaves it, so ordinary cells never pay
//! for it.
//!
//! # Contract
//!
//! - [`sync`] is called from every `bind`. When `is_editing` is false it must
//!   stay a single child lookup, all widget and closure construction lives in
//!   [`build_entry`], which only runs on the first bind of an editing cell.
//! - The entry carries its target path as widget data (`TARGET_PATH_KEY`),
//!   not in a closure capture. A recycled cell reuses the same entry for a
//!   different item, so the key handler must read the path at key time.
//! - Exactly one of {focus lost, Escape, Enter} ends an edit, and
//!   `submitted` stops focus-leave from sending a second `Refresh` after
//!   Escape or Enter already decided the outcome.
//! - [`remove`] is shared with `unbind` so a recycled cell never carries an
//!   entry over to the next item.

use crate::domain::model::{AppMsg, FileOpsMsg, NavigationMsg, SENDER};
use crate::ui::components::widgets::FileWidgets;
use crate::ui::components::FileItem;
use crate::ui::constants;
use adw::gdk;
use adw::prelude::*;
use gtk::glib;
use gtk::glib::clone;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const TARGET_PATH_KEY: &str = "target_rename_path";

/// Shows the entry when the item is being edited, restores the label otherwise.
pub(super) fn sync(item: &FileItem, widgets: &FileWidgets) {
    if item.is_editing {
        show(item, widgets);
    } else {
        hide(widgets);
    }
}

/// Removes the entry, if any. Does not touch which stack page is visible.
pub(super) fn remove(widgets: &FileWidgets) {
    if let Some(existing_entry) = widgets.stack.child_by_name(constants::VIEW_ENTRY) {
        widgets.stack.remove(&existing_entry);
    }
}

fn show(item: &FileItem, widgets: &FileWidgets) {
    crate::hit!("bind_rename_entry");
    let entry = match widgets.stack.child_by_name(constants::VIEW_ENTRY) {
        Some(w) => w.downcast::<gtk::Entry>().unwrap(),
        None => {
            let entry = build_entry();
            widgets.stack.add_named(&entry, Some(constants::VIEW_ENTRY));
            entry
        }
    };

    // Always refresh the target: the factory may reuse the entry for a new item.
    set_target_path(&entry, &item.path);

    widgets.stack.set_visible_child_name(constants::VIEW_ENTRY);
    entry.set_text(&item.name);
    entry.select_region(0, stem_len_chars(&item.name));

    gtk::glib::idle_add_local_once(clone!(
        #[weak]
        entry,
        move || {
            entry.grab_focus();
        }
    ));
}

fn hide(widgets: &FileWidgets) {
    crate::hit!("bind_rename_teardown");
    // Reclaim memory if a temporary Entry was previously instantiated
    remove(widgets);
    widgets.stack.set_visible_child_name(constants::VIEW_LABEL);
}

/// Length of the name without its extension, in chars (GTK selects by char).
fn stem_len_chars(name: &str) -> i32 {
    name.rfind('.')
        .map(|byte_idx| name[..byte_idx].chars().count())
        .unwrap_or_else(|| name.chars().count()) as i32
}

fn set_target_path(entry: &gtk::Entry, path: &Path) {
    unsafe {
        entry.set_data(TARGET_PATH_KEY, path.to_path_buf());
    }
}

/// Sends `msg` to the app. Returns whether a sender was available.
fn send(msg: AppMsg) -> bool {
    match SENDER.get() {
        Some(s) => {
            s.send(msg).ok();
            true
        }
        None => false,
    }
}

/// Builds the entry with its focus and key controllers wired.
///
/// The target path is not set here, [`show`] sets it on every bind.
fn build_entry() -> gtk::Entry {
    let entry = gtk::Entry::builder()
        .halign(gtk::Align::Center)
        .css_classes([constants::RENAME_ENTRY_CLASS])
        .build();

    let submitted = Rc::new(Cell::new(false));

    let focus_ctrl = gtk::EventControllerFocus::new();
    let submitted_focus = submitted.clone();
    focus_ctrl.connect_leave(move |_| {
        if !submitted_focus.get() {
            send(AppMsg::Navigation(NavigationMsg::Refresh));
        }
    });
    entry.add_controller(focus_ctrl);

    let key_ctrl = gtk::EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);

    let submitted_key = submitted;
    key_ctrl.connect_key_pressed(clone!(
        #[weak]
        entry,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                submitted_key.set(true);
                if send(AppMsg::Navigation(NavigationMsg::Refresh)) {
                    return glib::Propagation::Stop;
                }
            } else if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
                submitted_key.set(true);
                let target_path = unsafe {
                    entry
                        .data::<PathBuf>(TARGET_PATH_KEY)
                        .map(|p| p.as_ref().clone())
                };

                if let Some(old_path) = target_path {
                    let new_name = entry.text().to_string();
                    if send(AppMsg::FileOps(FileOpsMsg::PerformRename(
                        old_path, new_name,
                    ))) {
                        return glib::Propagation::Stop;
                    }
                }
            }
            glib::Propagation::Proceed
        }
    ));
    entry.add_controller(key_ctrl);

    entry
}
