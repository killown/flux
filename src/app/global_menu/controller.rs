//! Wiring of the global menu: startup binding, republishing and teardown.
//! Inert unless `ui.export_global_menu` is on and the compositor offers an
//! appmenu protocol (see [`super::protocol`]).
//!
//! The published menu is the application menubar (File, Edit, View, Go, Help).

use super::protocol::{
    bind_surface, detect_capability, unbind, update_bound_menu, AppMenuBinding, AppMenuCapability,
};
use crate::app::menubar::build_menubar_model;
use adw::gio;
use adw::prelude::*;
use gtk::glib;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

/// Bumped on every `republish`, a queued publish with an older value is
/// dropped, so a burst of changes publishes only the last model.
static GENERATION: AtomicU64 = AtomicU64::new(0);

struct State {
    binding: AppMenuBinding,
    /// The menubar model. `menubar::sync_recent_folders` edits it in place.
    menubar: gio::Menu,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Whether a binding is live.
fn is_bound() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

/// Binds the active window and publishes the menubar.
///
/// Does nothing when the option is off. The work runs from a timer, never on
/// the startup path, because the D-Bus export does a synchronous bus lookup.
pub fn init() {
    if !crate::utils::load_config().ui.export_global_menu {
        return;
    }
    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
        let capability = detect_capability();
        if capability == AppMenuCapability::None {
            return;
        }
        let app = relm4::main_adw_application();
        let Some(window) = app.active_window() else {
            return;
        };
        let binding = AppMenuBinding::new(capability, u64::from(std::process::id()));
        let menubar = build_menubar_model(&crate::utils::load_config().shortcuts);
        if let Err(e) = bind_surface(&binding, &window, &menubar) {
            eprintln!("[global-menu] bind failed: {e:?}");
            unbind(&binding);
            return;
        }
        window.connect_close_request(|_| {
            shutdown();
            glib::Propagation::Proceed
        });
        STATE.with(|s| *s.borrow_mut() = Some(State { binding, menubar }));
    });
}

/// Publishes the current menu from an idle callback, unless a newer request
/// arrived in the meantime. Call it after the menubar model or the state of
/// its actions changed. No-op when nothing is bound.
pub fn republish() {
    if !is_bound() {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    glib::idle_add_local_once(move || {
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        STATE.with(|s| {
            if let Some(st) = s.borrow().as_ref() {
                update_bound_menu(&st.binding, &st.menubar);
            }
        });
    });
}

/// Unbinds and unexports the D-Bus path. Safe to call when never bound.
pub fn shutdown() {
    if let Some(st) = STATE.with(|s| s.borrow_mut().take()) {
        unbind(&st.binding);
    }
}
