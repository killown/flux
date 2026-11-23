//! Static application menubar (global-menu).
//!
//! One declarative table ([`SECTIONS`]) drives three things, so they cannot
//! drift apart:
//!
//! * the `gio::Menu` model returned by [`build_menubar_model`],
//! * the `gio::SimpleAction`s registered by [`register_actions`],
//! * the accelerator hints shown next to each entry.
//!

use crate::domain::model::{
    AppMsg, AppSidebarMsg, FileOpsMsg, NavigationMsg, PreviewMsg, SearchMsg, ShortcutsConfig,
    ViewMsg,
};
use crate::i18n::tr;
use crate::ui::keymap::{constants, parse_accel_opt};
use adw::gio;
use adw::prelude::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Action prefix used by every action this module registers itself.
const PREFIX: &str = "mb-";
/// Name of the (always disabled) placeholder action.
const NOOP: &str = "mb-noop";
/// Parameterised action behind the "Recent Folders" entries. The parameter is
/// the 0-based position in `FluxApp::recent_stack`.
const JUMP_RECENT: &str = "mb-jump-recent";
/// Stateful radio action behind View ▸ View Mode.
const VIEW_MODE: &str = "mb-view-mode";

/// Edit actions whose enabled state follows the selection count.
///
/// `Paste` is deliberately absent: it targets the current folder, not the
/// selection, so disabling it with nothing selected would break the common
/// "copy, click empty space, paste" flow. Add `"mb-paste"` here if the strict
/// "every Edit entry is disabled at zero selection" reading is wanted.
const SELECTION_GATED: &[&str] = &["mb-copy", "mb-cut", "mb-rename", "mb-delete"];

fn send(msg: AppMsg) {
    if let Some(s) = crate::infra::SENDER.get() {
        let _ = s.send(msg);
    }
}

// ───────────────────────────── declarative table ─────────────────────────────

/// How an entry's accelerator hint is resolved.
#[derive(Clone, Copy)]
enum Accel {
    None,
    /// `(config field, KeyMap fallback)`.
    Cfg(fn(&ShortcutsConfig) -> &Option<String>, &'static str),
}

/// What activating an entry does.
#[derive(Clone, Copy)]
enum Target {
    /// Action already registered elsewhere, the menu only points at it.
    Existing,
    /// Registered here as a plain action running `fn()`.
    Run(fn()),
}

struct Item {
    /// Action name without the `app.` prefix.
    action: &'static str,
    label: &'static str,
    accel: Accel,
    target: Target,
}

const fn item(action: &'static str, label: &'static str, accel: Accel, target: Target) -> Item {
    Item {
        action,
        label,
        accel,
        target,
    }
}

static FILE_ITEMS: &[Item] = &[
    // `new-window` is bound to `shortcuts.new_tab` with a `<Primary>n`
    // default in `cli/shortcuts.rs`, mirrored here on purpose.
    item(
        "new-window",
        "New Window",
        Accel::Cfg(|s| &s.new_tab, "<Primary>n"),
        Target::Existing,
    ),
    item(
        "mb-new-folder",
        "New Folder",
        Accel::None,
        Target::Run(|| send(AppMsg::FileOps(FileOpsMsg::PromptNewFolder))),
    ),
    item(
        "mb-new-file",
        "New File",
        Accel::None,
        Target::Run(|| send(AppMsg::FileOps(FileOpsMsg::PromptNewFile))),
    ),
    item(
        "mb-reload",
        "Reload",
        Accel::Cfg(|s| &s.refresh, constants::REFRESH),
        Target::Run(|| send(AppMsg::Navigation(NavigationMsg::Refresh))),
    ),
    item(
        "mb-close-window",
        "Close Window",
        Accel::None,
        Target::Run(close_active_window),
    ),
    item(
        "mb-quit",
        "Quit",
        Accel::Cfg(|s| &s.quit, constants::QUIT),
        Target::Run(quit_app),
    ),
];

static EDIT_ITEMS: &[Item] = &[
    item(
        "mb-copy",
        "Copy",
        Accel::Cfg(|s| &s.copy, constants::COPY),
        Target::Run(|| send(AppMsg::FileOps(FileOpsMsg::Copy))),
    ),
    item(
        "mb-cut",
        "Cut",
        Accel::Cfg(|s| &s.cut, constants::CUT),
        Target::Run(|| send(AppMsg::FileOps(FileOpsMsg::Cut))),
    ),
    item(
        "mb-paste",
        "Paste",
        Accel::Cfg(|s| &s.paste, constants::PASTE),
        Target::Run(|| send(AppMsg::FileOps(FileOpsMsg::Paste))),
    ),
    item(
        "mb-rename",
        "Rename",
        Accel::Cfg(|s| &s.rename, "F2"),
        Target::Run(|| send(AppMsg::FileOps(FileOpsMsg::TriggerRenameSelection))),
    ),
    item(
        "mb-delete",
        "Delete",
        Accel::Cfg(|s| &s.delete, constants::DELETE),
        Target::Run(|| send(AppMsg::FileOps(FileOpsMsg::Delete))),
    ),
];

/// View entries before the View Mode radio pair.
static VIEW_ITEMS_A: &[Item] = &[
    item(
        "mb-toggle-hidden",
        "Toggle Hidden Files",
        Accel::Cfg(|s| &s.toggle_hidden, constants::TOGGLE_HIDDEN),
        Target::Run(|| send(AppMsg::View(ViewMsg::ToggleHidden))),
    ),
    item(
        "mb-toggle-folders-first",
        "Folders First",
        Accel::Cfg(|s| &s.toggle_folders_first, constants::TOGGLE_FOLDERS_FIRST),
        Target::Run(|| send(AppMsg::View(ViewMsg::ToggleCurrentFoldersFirst))),
    ),
];

/// View entries after the View Mode radio pair.
static VIEW_ITEMS_B: &[Item] = &[
    item(
        "mb-toggle-sidebar",
        "Show/Hide Sidebar",
        Accel::Cfg(|s| &s.toggle_sidebar, constants::TOGGLE_SIDEBAR),
        Target::Run(|| send(AppMsg::Sidebar(AppSidebarMsg::ToggleSidebar))),
    ),
    item(
        "mb-toggle-preview",
        "Show/Hide Preview",
        Accel::None,
        Target::Run(|| send(AppMsg::Preview(PreviewMsg::TogglePreview))),
    ),
    item(
        "open-settings",
        "Preferences",
        Accel::Cfg(|s| &s.settings, constants::SETTINGS),
        Target::Existing,
    ),
];

/// Go entries before the "Recent Folders" submenu.
static GO_ITEMS: &[Item] = &[
    item(
        "mb-back",
        "Back",
        Accel::Cfg(|s| &s.back, constants::BACK),
        Target::Run(|| send(AppMsg::Navigation(NavigationMsg::GoBack))),
    ),
    item(
        "mb-forward",
        "Forward",
        Accel::Cfg(|s| &s.forward, constants::FORWARD),
        Target::Run(|| send(AppMsg::Navigation(NavigationMsg::GoForward))),
    ),
    item(
        "mb-home",
        "Home",
        Accel::Cfg(|s| &s.home, constants::HOME),
        Target::Run(go_home),
    ),
    item(
        "mb-search",
        "Search",
        Accel::Cfg(|s| &s.search, constants::SEARCH),
        Target::Run(open_search),
    ),
];

static HELP_ITEMS: &[Item] = &[
    item(
        "open-help",
        "Keyboard Shortcuts",
        Accel::None,
        Target::Existing,
    ),
    item("show-about", "About", Accel::None, Target::Existing),
];

/// Every plain item table. The top-level labels and the special View / Go
/// entries (radio pair, recent-folders submenu) are assembled by hand in
/// [`build_menubar_model`], this list exists so `register_actions` and the
/// tests see every plain item exactly once.
static SECTIONS: &[&[Item]] = &[
    FILE_ITEMS,
    EDIT_ITEMS,
    VIEW_ITEMS_A,
    VIEW_ITEMS_B,
    GO_ITEMS,
    HELP_ITEMS,
];

// ─────────────────────────────── small actions ───────────────────────────────

fn close_active_window() {
    if let Some(app) = gio::Application::default() {
        if let Some(app) = app.downcast_ref::<gtk::Application>() {
            if let Some(win) = app.active_window() {
                win.close();
            }
        }
    }
}

fn quit_app() {
    if let Some(app) = gio::Application::default() {
        app.quit();
    }
}

fn go_home() {
    if let Some(home) = dirs::home_dir() {
        send(AppMsg::Navigation(NavigationMsg::Navigate(home)));
    }
}

/// Same two messages the search shortcut sends in `ui/inputs/shortcuts.rs`.
fn open_search() {
    send(AppMsg::Search(SearchMsg::ToggleSearchPanel));
    send(AppMsg::Search(SearchMsg::SwitchHeader(
        crate::domain::model::constants::VIEW_SEARCH.to_string(),
    )));
}

// ───────────────────────────────── model ─────────────────────────────────────

thread_local! {
    /// Actions flipped by [`sync_menubar_state`].
    static GATED: RefCell<Vec<gio::SimpleAction>> = const { RefCell::new(Vec::new()) };
    /// The "Recent Folders" submenu, kept so it can be repopulated in place.
    static RECENT_MENU: RefCell<Option<gio::Menu>> = const { RefCell::new(None) };
    /// Last list rendered into `RECENT_MENU`, to skip no-op rebuilds.
    static RECENT_SHOWN: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    /// The View Mode radio action.
    static VIEW_MODE_ACTION: RefCell<Option<gio::SimpleAction>> = const { RefCell::new(None) };
}

/// First chord of a `"a|b"` alternatives list.
///
/// `KeyMap` accepts `|` alternatives, the `accel` attribute takes one chord.
fn first_chord(list: &str) -> Option<&str> {
    list.split('|')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Whether GTK accepts `chord`. Without an initialised GTK (unit tests have no
/// display) `accelerator_parse` would panic, so the chord is taken as valid.
fn chord_ok(chord: &str) -> bool {
    !gtk::is_initialized() || gtk::accelerator_parse(chord).is_some()
}

/// Accelerator hint for one entry: the user's chord (or the fallback), first
/// alternative only. `None` when the shortcut is `"disabled"`. A chord GTK
/// rejects falls back to `default`, the same way `KeyMap` does.
fn display_accel(user: &Option<String>, default: &str) -> Option<String> {
    let resolved = parse_accel_opt(user, default)?;
    let first = first_chord(&resolved)?;
    if chord_ok(first) {
        return Some(first.to_string());
    }
    first_chord(default)
        .filter(|d| chord_ok(d))
        .map(str::to_string)
}

fn make_item(it: &Item, cfg: &ShortcutsConfig) -> gio::MenuItem {
    let mi = gio::MenuItem::new(Some(&tr(it.label)), Some(&format!("app.{}", it.action)));
    if let Accel::Cfg(field, default) = it.accel {
        if let Some(a) = display_accel(field(cfg), default) {
            mi.set_attribute_value("accel", Some(&a.to_variant()));
        }
    }
    mi
}

fn section_of(items: &[Item], cfg: &ShortcutsConfig) -> gio::Menu {
    let m = gio::Menu::new();
    for it in items {
        m.append_item(&make_item(it, cfg));
    }
    m
}

fn radio_item(label: &str, target: &str) -> gio::MenuItem {
    let mi = gio::MenuItem::new(Some(&tr(label)), None);
    mi.set_action_and_target_value(
        Some(&format!("app.{VIEW_MODE}")),
        Some(&target.to_variant()),
    );
    mi
}

fn recent_item(index: usize, path: &Path) -> gio::MenuItem {
    let label = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let mi = gio::MenuItem::new(Some(&label), None);
    mi.set_action_and_target_value(
        Some(&format!("app.{JUMP_RECENT}")),
        Some(&(index as u32).to_variant()),
    );
    mi
}

fn fill_recent(menu: &gio::Menu, stack: &[PathBuf]) {
    menu.remove_all();
    if stack.is_empty() {
        menu.append_item(&gio::MenuItem::new(
            Some(&tr("No recent folders")),
            Some(&format!("app.{NOOP}")),
        ));
        return;
    }
    for (i, p) in stack.iter().enumerate() {
        menu.append_item(&recent_item(i, p));
    }
}

/// Builds the top-level menubar model. Pure and headless, call once.
pub fn build_menubar_model(cfg: &ShortcutsConfig) -> gio::Menu {
    let bar = gio::Menu::new();

    bar.append_submenu(Some(&tr("File")), &section_of(FILE_ITEMS, cfg));
    bar.append_submenu(Some(&tr("Edit")), &section_of(EDIT_ITEMS, cfg));

    // View: toggles | view mode (radio) | sidebar/preview/preferences
    let view = gio::Menu::new();
    view.append_section(None, &section_of(VIEW_ITEMS_A, cfg));
    let modes = gio::Menu::new();
    modes.append_item(&radio_item("Grid View", "grid"));
    modes.append_item(&radio_item("List View", "list"));
    view.append_section(Some(&tr("View Mode")), &modes);
    view.append_section(None, &section_of(VIEW_ITEMS_B, cfg));
    bar.append_submenu(Some(&tr("View")), &view);

    // Go: nav entries, then the recent-folders submenu.
    let go = gio::Menu::new();
    go.append_section(None, &section_of(GO_ITEMS, cfg));
    let recent = gio::Menu::new();
    fill_recent(&recent, &[]);
    go.append_submenu(Some(&tr("Recent Folders")), &recent);
    RECENT_MENU.with(|r| *r.borrow_mut() = Some(recent));
    RECENT_SHOWN.with(|r| r.borrow_mut().clear());
    bar.append_submenu(Some(&tr("Go")), &go);

    bar.append_submenu(Some(&tr("Help")), &section_of(HELP_ITEMS, cfg));
    bar
}

// ───────────────────────────────── actions ───────────────────────────────────

/// Registers every action this module owns. Idempotent per `ActionMap`
/// (re-adding a name replaces the previous action).
pub fn register_actions(map: &impl IsA<gio::ActionMap>) {
    let mut gated = Vec::new();

    for items in SECTIONS {
        for it in items.iter() {
            let Target::Run(f) = it.target else { continue };
            debug_assert!(
                it.action.starts_with(PREFIX),
                "{} lacks {PREFIX}",
                it.action
            );
            let action = gio::SimpleAction::new(it.action, None);
            action.connect_activate(move |_, _| f());
            if SELECTION_GATED.contains(&it.action) {
                action.set_enabled(false); // zero selection at startup
                gated.push(action.clone());
            }
            map.add_action(&action);
        }
    }
    GATED.with(|g| *g.borrow_mut() = gated);

    // Placeholder entry for an empty recent list: always disabled.
    let noop = gio::SimpleAction::new(NOOP, None);
    noop.set_enabled(false);
    map.add_action(&noop);

    // Recent folders: the parameter is the 0-based index into
    // `FluxApp::recent_stack`. `NavigationMsg::JumpToRecent` takes a 1-based
    // rank (0 means "the most recent entry, no same-folder check", used by the
    // mouse button), so the index is shifted by one.
    let jump = gio::SimpleAction::new(JUMP_RECENT, Some(glib_u32_ty()));
    jump.connect_activate(|_, param| {
        if let Some(idx) = param.and_then(|p| p.get::<u32>()) {
            send(AppMsg::Navigation(NavigationMsg::JumpToRecent(
                idx as usize + 1,
            )));
        }
    });
    map.add_action(&jump);

    // View Mode radio. Only `ToggleListMode` exists, so activation toggles
    // when the requested mode differs from the current one.
    let list = crate::utils::load_config().default_list_mode;
    let mode = gio::SimpleAction::new_stateful(
        VIEW_MODE,
        Some(gtk::glib::VariantTy::STRING),
        &mode_key(list).to_variant(),
    );
    mode.connect_activate(|action, param| {
        let Some(want) = param.and_then(|p| p.str().map(str::to_owned)) else {
            return;
        };
        let current = action.state().and_then(|s| s.str().map(str::to_owned));
        if current.as_deref() != Some(want.as_str()) {
            action.set_state(&want.to_variant());
            send(AppMsg::View(ViewMsg::ToggleListMode));
        }
    });
    map.add_action(&mode);
    VIEW_MODE_ACTION.with(|a| *a.borrow_mut() = Some(mode));
}

fn glib_u32_ty() -> &'static gtk::glib::VariantTy {
    gtk::glib::VariantTy::UINT32
}

fn mode_key(list: bool) -> &'static str {
    if list {
        "list"
    } else {
        "grid"
    }
}

/// Builds the menubar, registers its actions and calls `set_menubar`.
///
/// `gtk::Application::set_menubar` must run after the application is
/// registered, so call this from the `startup` signal (see `cli/launch.rs`),
/// after `setup_shortcuts` has registered `new-window` / `open-settings` /
/// `open-help`.
///
/// With `ui.export_global_menu` on, `set_menubar` is skipped: GTK would
/// annotate the window with `org.gtk.Menus:menubar` and
/// `org.gtk.Actions:application`, which global-menu panels that read
/// `com.canonical.dbusmenu` do not understand. The actions stay registered and
/// `global_menu::controller` publishes the same model as a dbusmenu.
pub fn install(app: &adw::Application) {
    let cfg = crate::utils::load_config();
    register_actions(app);
    if !cfg.ui.export_global_menu {
        let model = build_menubar_model(&cfg.shortcuts);
        app.set_menubar(Some(&model));
    }
}

// ─────────────────────────────── state updates ───────────────────────────────

/// Enables the Edit entries when something is selected, disables them
/// otherwise. Entries are never hidden.
pub fn sync_menubar_state(selection_count: usize) {
    let enabled = selection_count > 0;
    GATED.with(|g| {
        for a in g.borrow().iter() {
            if a.is_enabled() != enabled {
                a.set_enabled(enabled);
            }
        }
    });
}

/// Mirrors `FluxApp::is_list_mode` into the View Mode radio, without
/// re-emitting a toggle. Call wherever the mode changes.
pub fn sync_view_mode(is_list: bool) {
    VIEW_MODE_ACTION.with(|a| {
        if let Some(a) = a.borrow().as_ref() {
            a.set_state(&mode_key(is_list).to_variant());
        }
    });
}

/// Repopulates Go ▸ Recent Folders from `FluxApp::recent_stack`.
/// Cheap no-op when the list is unchanged.
pub fn sync_recent_folders(stack: &VecDeque<PathBuf>) {
    let unchanged = RECENT_SHOWN.with(|s| s.borrow().iter().eq(stack.iter()));
    if unchanged {
        return;
    }
    let list: Vec<PathBuf> = stack.iter().cloned().collect();
    RECENT_MENU.with(|m| {
        if let Some(menu) = m.borrow().as_ref() {
            fill_recent(menu, &list);
        }
    });
    RECENT_SHOWN.with(|s| *s.borrow_mut() = list);
    crate::app::global_menu::controller::republish();
}

// ──────────────────────────────────── tests ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_actions(model: &gio::MenuModel, out: &mut Vec<String>) {
        for i in 0..model.n_items() {
            if let Some(v) = model.item_attribute_value(i, "action", None) {
                if let Some(s) = v.str() {
                    out.push(s.to_owned());
                }
            }
            for link in ["submenu", "section"] {
                if let Some(child) = model.item_link(i, link) {
                    collect_actions(&child, out);
                }
            }
        }
    }

    fn labels(model: &gio::MenuModel) -> Vec<String> {
        (0..model.n_items())
            .filter_map(|i| {
                model
                    .item_attribute_value(i, "label", None)
                    .and_then(|v| v.str().map(str::to_owned))
            })
            .collect()
    }

    #[test]
    fn top_level_order_is_file_edit_view_go_help() {
        let bar = build_menubar_model(&ShortcutsConfig::default());
        let l = labels(bar.upcast_ref());
        assert_eq!(l, ["File", "Edit", "View", "Go", "Help"].map(tr));
    }

    #[test]
    fn every_model_action_is_registered_or_known_existing() {
        let bar = build_menubar_model(&ShortcutsConfig::default());
        let group = gio::SimpleActionGroup::new();
        register_actions(&group);

        let existing: Vec<&str> = SECTIONS
            .iter()
            .flat_map(|s| s.iter())
            .filter(|i| matches!(i.target, Target::Existing))
            .map(|i| i.action)
            .collect();

        let mut names = Vec::new();
        collect_actions(bar.upcast_ref(), &mut names);
        assert!(!names.is_empty());
        for n in names {
            let bare = n.strip_prefix("app.").expect("app. prefix");
            assert!(
                group.has_action(bare) || existing.contains(&bare),
                "menu references unregistered action {bare}"
            );
        }
    }

    #[test]
    fn edit_entries_follow_selection_count() {
        let group = gio::SimpleActionGroup::new();
        register_actions(&group);

        let enabled = |n: &str| group.is_action_enabled(n);
        for n in SELECTION_GATED {
            assert!(!enabled(n), "{n} must start disabled");
        }
        sync_menubar_state(1);
        for n in SELECTION_GATED {
            assert!(enabled(n), "{n} must enable with a selection");
        }
        sync_menubar_state(0);
        for n in SELECTION_GATED {
            assert!(!enabled(n), "{n} must disable again");
        }
        // Paste is not selection-gated.
        assert!(enabled("mb-paste"));
    }

    #[test]
    fn disabled_shortcut_has_no_accel_hint() {
        let cfg = ShortcutsConfig {
            delete: Some("disabled".into()),
            ..Default::default()
        };
        assert_eq!(display_accel(&cfg.delete, constants::DELETE), None);
        assert_eq!(
            display_accel(&None, constants::DELETE).as_deref(),
            Some("Delete")
        );
        // `|` alternatives: first chord wins.
        assert_eq!(
            display_accel(&None, constants::REDO).as_deref(),
            Some("<ctrl><shift>z")
        );
    }

    #[test]
    fn recent_folders_rebuild_and_empty_state() {
        let _ = build_menubar_model(&ShortcutsConfig::default());
        let mut stack = VecDeque::new();
        stack.push_back(PathBuf::from("/home/u/a"));
        stack.push_back(PathBuf::from("/tmp"));
        sync_recent_folders(&stack);
        RECENT_MENU.with(|m| {
            assert_eq!(m.borrow().as_ref().unwrap().n_items(), 2);
        });
        sync_recent_folders(&VecDeque::new());
        RECENT_MENU.with(|m| {
            // single disabled placeholder
            assert_eq!(m.borrow().as_ref().unwrap().n_items(), 1);
        });
    }

    #[test]
    fn recent_items_carry_zero_based_index() {
        let mi = recent_item(3, Path::new("/tmp/x"));
        let target = mi
            .attribute_value("target", None)
            .and_then(|v| v.get::<u32>());
        assert_eq!(target, Some(3));
    }
}
