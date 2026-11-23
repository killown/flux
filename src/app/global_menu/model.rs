//! `GMenuModel` builder for the dynamic (per-selection) global menu.
//!
//! Everything here is plain `gio::Menu` data and pure functions over
//! [`CustomAction`], so it needs no window and never touches the main loop.
//! The context popover is untouched: this module only reads the same action
//! list and the same MIME matcher the popover uses.

use crate::domain::model::constants::MIME_DIR;
use crate::domain::model::CustomAction;
use crate::i18n::tr;
use crate::ui::menu::secondary::action_matches_mime;
use crate::utils::glob::{compile_patterns, expand_mime_category};
use adw::gio;
use std::path::PathBuf;

/// Prefix of the `GAction` every entry points at. `controller` registers one
/// action per `CustomAction::action_name` under this prefix.
pub const ACTION_PREFIX: &str = "app.gm-";

/// Disabled placeholder action registered by `menubar::register_actions`.
const NOOP_ACTION: &str = "app.mb-noop";

/// One selected item, as far as action filtering is concerned.
#[derive(Clone, Debug)]
pub struct SelectionInfo {
    pub path: PathBuf,
    /// Detected MIME type, `inode/directory` for folders.
    pub mime: String,
}

fn menu_item(action: &CustomAction) -> gio::MenuItem {
    // Labels are already translated by `load_menu_config`, no second `tr()`.
    gio::MenuItem::new(
        Some(&action.label),
        Some(&format!("{ACTION_PREFIX}{}", action.action_name)),
    )
}

/// Builds the secondary model from an already filtered action list.
///
/// Entries without a `submenu` sit at the top level. Each distinct `submenu`
/// becomes one nested menu, placed where it first appears. Both keep the order
/// `load_menu_config` returned. Labels and submenu names are used as given.
pub fn build_secondary_model(actions: &[CustomAction]) -> gio::Menu {
    let root = gio::Menu::new();
    let mut submenus: Vec<(&str, gio::Menu)> = Vec::new();

    for action in actions {
        let item = menu_item(action);
        match action.submenu.as_deref() {
            None => root.append_item(&item),
            Some(name) => {
                let idx = match submenus.iter().position(|(n, _)| *n == name) {
                    Some(i) => i,
                    None => {
                        let menu = gio::Menu::new();
                        root.append_submenu(Some(name), &menu);
                        submenus.push((name, menu));
                        submenus.len() - 1
                    }
                };
                submenus[idx].1.append_item(&item);
            }
        }
    }
    root
}

/// Whether `action` also matches `path` by file name, using the glob
/// expansion of its `mime_types` entries (`image/*`, `doc/*`, `*.rs`, ...).
fn action_matches_name(action: &CustomAction, path: &std::path::Path) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    let patterns: Vec<String> = action
        .mime_types
        .iter()
        .flat_map(|entry| entry.split('+'))
        .map(str::trim)
        .filter(|req| req.contains('/') || req.contains('*') || req.contains('.'))
        .flat_map(expand_mime_category)
        .collect();
    compile_patterns(&patterns).is_some_and(|set| set.is_match(name))
}

/// Actions that apply to the current selection, in their original order.
///
/// An action is kept when any selected item matches it, either by MIME (the
/// popover's `action_matches_mime`) or by file-name glob. An empty selection
/// yields an empty list, use [`empty_selection_model`] for that state.
pub fn filter_actions_for_selection(
    actions: &[CustomAction],
    selected: &[SelectionInfo],
) -> Vec<CustomAction> {
    if selected.is_empty() {
        return Vec::new();
    }
    actions
        .iter()
        .filter(|a| {
            selected.iter().any(|s| {
                action_matches_mime(a, &s.mime)
                    || (s.mime != MIME_DIR && action_matches_name(a, &s.path))
            })
        })
        .cloned()
        .collect()
}

/// Model for "nothing selected": one disabled "No selection" item.
pub fn empty_selection_model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_item(&gio::MenuItem::new(
        Some(&tr("No selection")),
        Some(NOOP_ACTION),
    ));
    menu
}
