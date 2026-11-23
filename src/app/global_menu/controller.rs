//! Wiring of the global menu: startup binding, selection updates and
//! teardown. Inert unless `ui.export_global_menu` is on and the compositor
//! offers an appmenu protocol (see [`super::protocol`]).
//!
//! The published menu is the application menubar (File, Edit, View, Go, Help)
//! followed by an "Actions" submenu with the entries that fit the selection.

use super::model::{
    build_secondary_model, empty_selection_model, filter_actions_for_selection, SelectionInfo,
    ACTION_PREFIX,
};
use super::protocol::{
    bind_surface, detect_capability, unbind, update_bound_menu, AppMenuBinding, AppMenuCapability,
};
use crate::app::menubar::build_menubar_model;
use crate::domain::model::constants::MIME_DIR;
use crate::domain::model::{ActionsMsg, AppMsg, CustomAction, WindowMsg};
use crate::i18n::tr;
use adw::gio;
use adw::prelude::*;
use gtk::glib;
use std::cell::RefCell;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Bumped on every selection change, a queued publish with an older value is
/// dropped, so a burst of arrow-key presses publishes only the last model.
static GENERATION: AtomicU64 = AtomicU64::new(0);

struct State {
    binding: AppMenuBinding,
    actions: Vec<CustomAction>,
    /// The menubar model. `menubar::sync_recent_folders` edits it in place.
    menubar: gio::Menu,
    /// Content of the "Actions" submenu, `None` when no action fits.
    actions_menu: Option<gio::Menu>,
}

/// The menubar followed by the "Actions" submenu. The root is a copy, the
/// submenus are shared with `menubar`.
fn compose(menubar: &gio::Menu, actions: Option<&gio::Menu>) -> gio::Menu {
    let root = gio::Menu::new();
    for i in 0..menubar.n_items() {
        root.append_item(&gio::MenuItem::from_model(menubar, i));
    }
    if let Some(actions) = actions {
        root.append_submenu(Some(&tr("Actions")), actions);
    }
    root
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Whether a binding is live. Cheap, lets callers skip building selection data.
pub fn is_bound() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

/// Selection data for the filter. The MIME comes from the file name only
/// (`content_type_guess` with no data), so it does no I/O on the GTK thread.
pub fn selection_info(path: &Path, is_dir: bool) -> SelectionInfo {
    let mime = if is_dir {
        MIME_DIR.to_string()
    } else {
        gio::content_type_guess(Some(path), None).0.to_string()
    };
    SelectionInfo {
        path: path.to_path_buf(),
        mime,
    }
}

/// Registers one `gm-<action_name>` action per `CustomAction`, activating it
/// runs the same message the Actions panel sends.
fn register_actions(app: &adw::Application, actions: &[CustomAction]) {
    let prefix = ACTION_PREFIX.trim_start_matches("app.");
    for a in actions {
        let action = gio::SimpleAction::new(&format!("{prefix}{}", a.action_name), None);
        let (command, no_dialog, toast) = (a.command.clone(), a.no_command_dialog, a.toast.clone());
        action.connect_activate(move |_, _| {
            if let Some(s) = crate::infra::SENDER.get() {
                let _ = s.send(AppMsg::Actions(ActionsMsg::ExecuteSecondaryAction {
                    command: command.clone(),
                    no_command_dialog: no_dialog,
                }));
                if let Some(msg) = &toast {
                    let _ = s.send(AppMsg::Window(WindowMsg::ShowToast(msg.clone())));
                }
            }
        });
        app.add_action(&action);
    }
}

/// Binds the active window and publishes the menubar with the "No selection"
/// actions menu.
///
/// Does nothing when the option is off. The work runs from a timer, never on
/// the startup path, because the D-Bus export does a synchronous bus lookup.
pub fn init(actions: Vec<CustomAction>) {
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
        register_actions(&app, &actions);
        let binding = AppMenuBinding::new(capability, u64::from(std::process::id()));
        let menubar = build_menubar_model(&crate::utils::load_config().shortcuts);
        let actions_menu = Some(empty_selection_model());
        let root = compose(&menubar, actions_menu.as_ref());
        if let Err(e) = bind_surface(&binding, &window, &root) {
            eprintln!("[global-menu] bind failed: {e:?}");
            unbind(&binding);
            return;
        }
        window.connect_close_request(|_| {
            shutdown();
            glib::Propagation::Proceed
        });
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                binding,
                actions,
                menubar,
                actions_menu,
            })
        });
    });
}

/// Rebuilds the "Actions" submenu for `selected` and publishes the menu.
pub fn on_selection_changed(selected: Vec<SelectionInfo>) {
    let bound = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(st) = s.as_mut() else { return false };
        st.actions_menu = if selected.is_empty() {
            Some(empty_selection_model())
        } else {
            let menu = build_secondary_model(&filter_actions_for_selection(&st.actions, &selected));
            (menu.n_items() > 0).then_some(menu)
        };
        true
    });
    if bound {
        republish();
    }
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
                update_bound_menu(&st.binding, &compose(&st.menubar, st.actions_menu.as_ref()));
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
