//! Quick List panel: every named list as a tree, with a fuzzy filter.
//!
//! ```text
//! Work                      3   +
//!     1  flux
//!     2  docs
//! Photos                    1   +
//!     portfolio
//! ```
//!
//! The panel is a view of `FluxApp`'s lists and holds no state of its own
//! beyond which lists are collapsed. `FluxApp::handle_rebuild_quick_panel`
//! pushes a fresh [`QuickGroups`] snapshot through [`refresh`], and every
//! click turns into an `AppMsg::QuickList*` message, so the data only ever
//! changes in the update loop.
//!
//! The selected list (bold, with a check mark) is the one Tab and
//! Ctrl+PageUp/PageDown walk. The first nine items of it are numbered for
//! Ctrl+1..9.

use super::header::panel_header;
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::domain::model::quick_groups::{GroupView, QuickGroups};
use crate::domain::model::{AppMsg, QuickListMsg, FluxApp};
use crate::i18n::tr;
use crate::utils::search::fuzzy_match_compiled;
use adw::prelude::*;
use gtk::{gio, glib};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

type Sender = AsyncComponentSender<FluxApp>;

/// What activating a row does. Index-aligned with the rows of the list box.
#[derive(Clone, Debug)]
enum RowAction {
    /// List header: select the list.
    Group { list: usize, name: String },
    /// Folder row: open it.
    Item { list: usize, path: PathBuf },
    /// Placeholder under an empty list. Does nothing.
    Hint,
}

/// The live widgets of the built panel.
#[derive(Clone)]
struct Handle {
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    stack: gtk::Stack,
    actions: Rc<RefCell<Vec<RowAction>>>,
}

thread_local! {
    static CURRENT: RefCell<Option<Handle>> = const { RefCell::new(None) };
    /// The last snapshot pushed by [`refresh`].
    static DATA: RefCell<Option<QuickGroups>> = const { RefCell::new(None) };
    /// Names of the lists folded shut. Ignored while a filter is typed.
    static COLLAPSED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Short name for a quick-list row.
///
/// An `archive://` location is shown by what it points at: the archive's file
/// name without its extension at the root, or the folder name inside it.
/// Anything else uses its last path component.
pub(crate) fn item_label(path: &Path) -> String {
    if let Some((archive, inner)) =
        crate::infra::archive::parse_archive_uri(&path.to_string_lossy())
    {
        if let Some(name) = inner.trim_matches('/').rsplit('/').next() {
            if !name.is_empty() {
                return name.to_string();
            }
        }
        if let Some(stem) = archive.file_stem().map(|s| s.to_string_lossy()) {
            // `.tar.gz` and friends: the stem still ends in `.tar`.
            return stem.strip_suffix(".tar").unwrap_or(&stem).to_string();
        }
    }
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Tooltip for a quick-list row: the real location, with an `archive://`
/// URI shown as the archive's path followed by the folder inside it.
pub(crate) fn item_tooltip(path: &Path) -> String {
    match crate::infra::archive::parse_archive_uri(&path.to_string_lossy()) {
        Some((archive, inner)) => archive
            .join(inner.trim_matches('/'))
            .to_string_lossy()
            .into(),
        None => path.to_string_lossy().into_owned(),
    }
}

/// Builds the Quick List panel and registers its widgets for [`refresh`].
pub fn build_quick_list_panel(initial_width: i32, sender: Sender) -> gtk::Box {
    crate::hit!("build_quick_list_panel");
    let spec = PanelSpec::quick_list().with_initial(initial_width);

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    let new_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("New list"))
        .build();
    {
        let s = sender.clone();
        new_btn.connect_clicked(move |_| prompt_new_list(&s));
    }
    let header = {
        let s = sender.clone();
        panel_header(
            Some("view-list-symbolic"),
            &tr("Quick List"),
            &[new_btn.upcast()],
            move || s.input(AppMsg::QuickList(QuickListMsg::ToggleQuickListPanel)),
        )
    };
    panel.append(&header);

    let search = gtk::SearchEntry::builder()
        .placeholder_text(tr("Filter lists and folders…"))
        .hexpand(true)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(8)
        .build();
    panel.append(&search);

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .activate_on_single_click(true)
        .margin_end(4)
        .build();

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .overlay_scrolling(false)
        .propagate_natural_width(false)
        .hexpand(false)
        .vexpand(true)
        .margin_start(8)
        .margin_end(4)
        .margin_bottom(12)
        .child(&list)
        .build();

    let no_match = adw::StatusPage::builder()
        .icon_name("edit-find-symbolic")
        .title(tr("No matches"))
        .vexpand(true)
        .build();

    let stack = gtk::Stack::builder().vexpand(true).build();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(&no_match, Some("empty"));
    stack.set_visible_child_name("list");
    panel.append(&stack);

    let actions: Rc<RefCell<Vec<RowAction>>> = Rc::new(RefCell::new(Vec::new()));

    // Click or Enter on a row.
    {
        let actions = actions.clone();
        let s = sender.clone();
        list.connect_row_activated(move |_, row| {
            let Ok(idx) = usize::try_from(row.index()) else {
                return;
            };
            let action = actions.borrow().get(idx).cloned();
            match action {
                Some(RowAction::Group { list, .. }) => s.input(AppMsg::QuickList(QuickListMsg::QuickListSelect(list))),
                Some(RowAction::Item { list, path }) => {
                    s.input(AppMsg::QuickList(QuickListMsg::QuickListOpen { list, path }))
                }
                Some(RowAction::Hint) | None => {}
            }
        });
    }

    // Left / Right fold a list header, Delete removes the focused folder.
    {
        let actions = actions.clone();
        let s = sender.clone();
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |ctrl, key, _, _| {
            let focused = ctrl
                .widget()
                .and_then(|w| w.root())
                .and_then(|r| r.focus())
                .and_then(|f| row_of(&f));
            let Some(row) = focused else {
                return glib::Propagation::Proceed;
            };
            let Ok(idx) = usize::try_from(row.index()) else {
                return glib::Propagation::Proceed;
            };
            let action = actions.borrow().get(idx).cloned();
            match (key, action) {
                (gtk::gdk::Key::Left, Some(RowAction::Group { name, .. })) => {
                    if set_collapsed(&name, true) {
                        render(&s);
                    }
                    glib::Propagation::Stop
                }
                (gtk::gdk::Key::Right, Some(RowAction::Group { name, .. })) => {
                    if set_collapsed(&name, false) {
                        render(&s);
                    }
                    glib::Propagation::Stop
                }
                (
                    gtk::gdk::Key::Delete | gtk::gdk::Key::KP_Delete,
                    Some(RowAction::Item { list, path }),
                ) => {
                    s.input(AppMsg::QuickList(QuickListMsg::QuickListRemove { list, path }));
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        list.add_controller(keys);
    }

    // Filter as you type. Enter opens the first folder left, Down moves into
    // the list, Esc clears the filter and then closes the panel.
    {
        let s = sender.clone();
        search.connect_search_changed(move |_| render(&s));
    }
    {
        let actions = actions.clone();
        let s = sender.clone();
        search.connect_activate(move |_| {
            let first = actions.borrow().iter().find_map(|a| match a {
                RowAction::Item { list, path } => Some((*list, path.clone())),
                _ => None,
            });
            if let Some((list, path)) = first {
                s.input(AppMsg::QuickList(QuickListMsg::QuickListOpen { list, path }));
            }
        });
    }
    {
        let s = sender.clone();
        search.connect_stop_search(move |entry| {
            if entry.text().is_empty() {
                s.input(AppMsg::QuickList(QuickListMsg::ToggleQuickListPanel));
            } else {
                entry.set_text("");
            }
        });
    }
    {
        let list = list.clone();
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Down {
                if let Some(first) = list.row_at_index(0) {
                    first.grab_focus();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        search.add_controller(keys);
    }

    CURRENT.with(|c| {
        *c.borrow_mut() = Some(Handle {
            search,
            list,
            stack,
            actions,
        })
    });

    resizable_panel(&spec, &panel, |_| {})
}

/// Stores a fresh snapshot and redraws the panel when it is built.
pub(crate) fn refresh(groups: QuickGroups, sender: &Sender) {
    crate::hit!("quick_list_panel_refresh");
    DATA.with(|d| *d.borrow_mut() = Some(groups));
    render(sender);
}

/// Puts the cursor in the filter entry.
pub(crate) fn focus_search() {
    let handle = CURRENT.with(|c| c.borrow().clone());
    if let Some(h) = handle {
        h.search.grab_focus();
    }
}

/// Folds or unfolds a list. `true` when the state changed.
fn set_collapsed(name: &str, collapsed: bool) -> bool {
    COLLAPSED.with(|c| {
        let mut set = c.borrow_mut();
        if collapsed {
            set.insert(name.to_string())
        } else {
            set.remove(name)
        }
    })
}

/// The `ListBoxRow` that `widget` is or sits in.
fn row_of(widget: &gtk::Widget) -> Option<gtk::ListBoxRow> {
    let mut cur = Some(widget.clone());
    while let Some(w) = cur {
        match w.clone().downcast::<gtk::ListBoxRow>() {
            Ok(row) => return Some(row),
            Err(_) => cur = w.parent(),
        }
    }
    None
}

/// Redraws every row from the last snapshot and the current filter.
fn render(sender: &Sender) {
    crate::hit!("quick_list_panel_render");
    let Some(h) = CURRENT.with(|c| c.borrow().clone()) else {
        return;
    };
    let Some(groups) = DATA.with(|d| d.borrow().clone()) else {
        return;
    };

    let query = h.search.text().to_string();
    let query = query.trim();
    let pattern = (!query.is_empty())
        .then(|| Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart));
    let filtering = pattern.is_some();

    let views = groups.views(
        |t| pattern.as_ref().is_none_or(|p| fuzzy_match_compiled(t, p)),
        |p| format!("{} {}", item_label(p), item_tooltip(p)),
    );

    while let Some(child) = h.list.first_child() {
        h.list.remove(&child);
    }

    let collapsed = COLLAPSED.with(|c| c.borrow().clone());
    let can_delete = groups.lists.len() > 1;
    let mut actions: Vec<RowAction> = Vec::new();

    for v in &views {
        let open = filtering || !collapsed.contains(&v.name);
        h.list
            .append(&group_row(v, open, filtering, can_delete, sender));
        actions.push(RowAction::Group {
            list: v.list,
            name: v.name.clone(),
        });
        if !open {
            continue;
        }

        if v.total == 0 {
            let hint = gtk::Label::builder()
                .label(tr("Empty. Press Insert to add the current folder."))
                .xalign(0.0)
                .wrap(true)
                .margin_start(32)
                .margin_top(2)
                .margin_bottom(6)
                .css_classes(["dim-label", "caption"])
                .build();
            let row = gtk::ListBoxRow::builder()
                .activatable(false)
                .selectable(false)
                .child(&hint)
                .build();
            h.list.append(&row);
            actions.push(RowAction::Hint);
            continue;
        }

        let others: Vec<(usize, String)> = groups
            .lists
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != v.list)
            .map(|(i, g)| (i, g.name.clone()))
            .collect();
        for (slot, path) in &v.rows {
            let is_current = v.selected && v.current == Some(*slot);
            h.list.append(&item_row(
                v.list, *slot, path, v.selected, is_current, &others, sender,
            ));
            actions.push(RowAction::Item {
                list: v.list,
                path: path.clone(),
            });
        }
    }

    *h.actions.borrow_mut() = actions;
    h.stack
        .set_visible_child_name(if views.is_empty() { "empty" } else { "list" });
}

/// Header row of one list.
fn group_row(
    v: &GroupView,
    open: bool,
    filtering: bool,
    can_delete: bool,
    sender: &Sender,
) -> gtk::ListBoxRow {
    crate::hit!("group_row");
    let row = gtk::ListBoxRow::builder().activatable(true).build();
    if v.selected {
        row.set_tooltip_text(Some(&tr("Tab walks this list")));
    }

    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .margin_start(2)
        .margin_end(4)
        .margin_top(2)
        .margin_bottom(2)
        .build();

    let arrow = gtk::Button::builder()
        .icon_name(if open {
            "pan-down-symbolic"
        } else {
            "pan-end-symbolic"
        })
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .sensitive(!filtering)
        .tooltip_text(if open { tr("Collapse") } else { tr("Expand") })
        .build();
    {
        let name = v.name.clone();
        let s = sender.clone();
        arrow.connect_clicked(move |_| {
            let fold = !COLLAPSED.with(|c| c.borrow().contains(&name));
            set_collapsed(&name, fold);
            // Rebuilding removes this very button, so wait for the click to finish.
            let s = s.clone();
            glib::idle_add_local_once(move || render(&s));
        });
    }
    line.append(&arrow);

    let name = gtk::Label::builder()
        .label(v.name.as_str())
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["heading"])
        .build();
    if v.selected {
        name.add_css_class("accent");
    }
    line.append(&name);

    if v.selected {
        line.append(&gtk::Image::from_icon_name("object-select-symbolic"));
    }

    line.append(
        &gtk::Label::builder()
            .label(v.total.to_string())
            .css_classes(["dim-label", "caption"])
            .build(),
    );

    let add_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Add current folder to this list"))
        .build();
    {
        let list = v.list;
        let s = sender.clone();
        add_btn.connect_clicked(move |_| s.input(AppMsg::QuickList(QuickListMsg::QuickListAddCurrent(list))));
    }
    line.append(&add_btn);

    let del_btn = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Delete this list"))
        .build();
    {
        let list = v.list;
        let list_name = v.name.clone();
        let total = v.total;
        let s = sender.clone();
        del_btn.connect_clicked(move |_| {
            let body = if total == 0 {
                format!("{} “{}”.", tr("This removes the list"), list_name)
            } else {
                format!(
                    "{} “{}”. {}",
                    tr("This removes the list"),
                    list_name,
                    tr("The folders themselves are not touched.")
                )
            };
            let s2 = s.clone();
            confirm(&tr("Delete List"), &body, &tr("Delete"), move || {
                s2.input(AppMsg::QuickList(QuickListMsg::QuickListDelete(list)));
            });
        });
    }
    line.append(&del_btn);

    row.set_child(Some(&line));

    {
        let list = v.list;
        let list_name = v.name.clone();
        let total = v.total;
        let s = sender.clone();
        attach_menu(&row, move || {
            let mut items = Vec::new();

            let (s1, n1) = (s.clone(), list_name.clone());
            items.push(MenuItem::Action(
                tr("Rename List…"),
                Box::new(move || {
                    let s2 = s1.clone();
                    prompt_name(
                        &tr("Rename List"),
                        &tr("Enter a new name for this list:"),
                        &n1,
                        &tr("Rename"),
                        move |name| {
                            s2.input(AppMsg::QuickList(QuickListMsg::QuickListRename { list, name }));
                        },
                    );
                }),
            ));

            let s1 = s.clone();
            items.push(MenuItem::Action(
                tr("Add Current Folder"),
                Box::new(move || s1.input(AppMsg::QuickList(QuickListMsg::QuickListAddCurrent(list)))),
            ));

            if total > 0 {
                let s1 = s.clone();
                items.push(MenuItem::Action(
                    tr("Clear List"),
                    Box::new(move || s1.input(AppMsg::QuickList(QuickListMsg::QuickListClearList(list)))),
                ));
            }

            if can_delete {
                let (s1, n1) = (s.clone(), list_name.clone());
                items.push(MenuItem::Action(
                    tr("Delete List"),
                    Box::new(move || {
                        if total == 0 {
                            s1.input(AppMsg::QuickList(QuickListMsg::QuickListDelete(list)));
                            return;
                        }
                        let s2 = s1.clone();
                        confirm(
                            &tr("Delete List"),
                            &format!(
                                "{} “{}”. {}",
                                tr("This removes the list"),
                                n1,
                                tr("The folders themselves are not touched.")
                            ),
                            &tr("Delete"),
                            move || s2.input(AppMsg::QuickList(QuickListMsg::QuickListDelete(list))),
                        );
                    }),
                ));
            }
            items
        });
    }

    row
}

/// One folder row.
fn item_row(
    list: usize,
    slot: usize,
    path: &Path,
    in_selected_list: bool,
    is_current: bool,
    others: &[(usize, String)],
    sender: &Sender,
) -> gtk::ListBoxRow {
    crate::hit!("item_row");
    let row = gtk::ListBoxRow::builder()
        .activatable(true)
        .tooltip_text(item_tooltip(path))
        .build();

    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(30)
        .margin_end(6)
        .margin_top(3)
        .margin_bottom(3)
        .build();

    // Ctrl+1..9 only reach the selected list, so only its rows carry a number.
    let number = if in_selected_list && slot < 9 {
        (slot + 1).to_string()
    } else {
        String::new()
    };
    line.append(
        &gtk::Label::builder()
            .label(number)
            .width_chars(1)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    line.append(&gtk::Image::from_icon_name("folder-symbolic"));

    let label = gtk::Label::builder()
        .label(item_label(path))
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    if is_current {
        label.add_css_class("accent");
    }
    line.append(&label);
    row.set_child(Some(&line));

    // Middle click removes the folder from its list.
    {
        let path = path.to_path_buf();
        let s = sender.clone();
        let middle = gtk::GestureClick::new();
        middle.set_button(2);
        middle.connect_pressed(move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            s.input(AppMsg::QuickList(QuickListMsg::QuickListRemove {
                list,
                path: path.clone(),
            }));
        });
        row.add_controller(middle);
    }

    // Right click menu.
    {
        let path = path.to_path_buf();
        let others = others.to_vec();
        let s = sender.clone();
        attach_menu(&row, move || {
            let mut items = Vec::new();

            let s1 = s.clone();
            items.push(MenuItem::Action(
                tr("Set to Current Folder"),
                Box::new(move || s1.input(AppMsg::QuickList(QuickListMsg::QuickListSetSlot { list, slot }))),
            ));

            let p1 = path.clone();
            items.push(MenuItem::Action(
                tr("Open in New Window"),
                Box::new(move || {
                    crate::utils::helpers::open_new_instance(&p1);
                }),
            ));

            if !others.is_empty() {
                let subs = others
                    .iter()
                    .map(|(to, name)| {
                        let (s1, p1, to) = (s.clone(), path.clone(), *to);
                        let f: Box<dyn Fn() + 'static> = Box::new(move || {
                            s1.input(AppMsg::QuickList(QuickListMsg::QuickListMove {
                                path: p1.clone(),
                                from: list,
                                to,
                            }))
                        });
                        (name.clone(), f)
                    })
                    .collect();
                items.push(MenuItem::Submenu(tr("Move to List"), subs));
            }

            let (s1, p1) = (s.clone(), path.clone());
            items.push(MenuItem::Action(
                tr("Remove from Quick List"),
                Box::new(move || {
                    s1.input(AppMsg::QuickList(QuickListMsg::QuickListRemove {
                        list,
                        path: p1.clone(),
                    }))
                }),
            ));
            items
        });
    }

    // Dropping files on a folder row moves them there, like the old dock buttons did.
    {
        let formats = gtk::gdk::ContentFormats::builder()
            .add_type(gtk::gdk::FileList::static_type())
            .build();
        let drop_target = gtk::DropTarget::builder()
            .actions(gtk::gdk::DragAction::MOVE | gtk::gdk::DragAction::COPY)
            .formats(&formats)
            .build();
        let target = path.to_path_buf();
        let s = sender.clone();
        drop_target.connect_drop(move |_target, value, _, _| {
            if let Ok(file_list) = value.get::<gtk::gdk::FileList>() {
                let sources: Vec<PathBuf> = file_list
                    .files()
                    .into_iter()
                    .filter_map(|f| f.path())
                    .collect();
                if !sources.is_empty() {
                    s.input(AppMsg::QuickList(QuickListMsg::MoveFilesToTarget {
                        sources,
                        destination: target.clone(),
                    }));
                    return true;
                }
            }
            false
        });
        row.add_controller(drop_target);
    }

    row
}

// ─── Menus and dialogs ───────────────────────────────────────────────────────

enum MenuItem {
    Action(String, Box<dyn Fn() + 'static>),
    Submenu(String, Vec<(String, Box<dyn Fn() + 'static>)>),
}

/// Right click on `row` pops up the menu `build` returns. `build` runs per
/// click, so it always sees the state of that moment.
fn attach_menu(row: &gtk::ListBoxRow, build: impl Fn() -> Vec<MenuItem> + 'static) {
    let right = gtk::GestureClick::new();
    right.set_button(3);
    right.connect_released(move |gesture, _, x, y| {
        if crate::utils::load_config().ui.panel_menu_requires_ctrl
            && !gesture
                .current_event_state()
                .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return;
        }
        gesture.set_state(gtk::EventSequenceState::Claimed);
        if let Some(widget) = gesture.widget() {
            popup_menu(&widget, x, y, build());
        }
    });
    row.add_controller(right);
}

fn popup_menu(widget: &gtk::Widget, x: f64, y: f64, items: Vec<MenuItem>) {
    crate::hit!("popup_menu");
    let menu = gio::Menu::new();
    let group = gio::SimpleActionGroup::new();
    let mut n = 0usize;
    let mut register = |parent: &gio::Menu, label: &str, f: Box<dyn Fn() + 'static>| {
        let name = format!("a{n}");
        n += 1;
        let act = gio::SimpleAction::new(&name, None);
        act.connect_activate(move |_, _| f());
        group.add_action(&act);
        parent.append(Some(label), Some(&format!("quickmenu.{name}")));
    };
    for item in items {
        match item {
            MenuItem::Action(label, f) => register(&menu, &label, f),
            MenuItem::Submenu(label, subs) => {
                let sub = gio::Menu::new();
                for (l, f) in subs {
                    register(&sub, &l, f);
                }
                menu.append_submenu(Some(&label), &sub);
            }
        }
    }

    let popover = gtk::PopoverMenu::builder().has_arrow(true).build();
    popover.set_menu_model(Some(&menu));
    popover.set_parent(widget);
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    widget.insert_action_group("quickmenu", Some(&group));
    // The rows are rebuilt on every change, so a popover left parented would
    // outlive its row's usefulness.
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
}

fn prompt_new_list(sender: &Sender) {
    let fallback = tr("New list");
    let suggestion = DATA
        .with(|d| d.borrow().as_ref().map(|g| g.unique_name(&fallback)))
        .unwrap_or(fallback);
    let s = sender.clone();
    prompt_name(
        &tr("New Quick List"),
        &tr("Enter a name for the new list:"),
        &suggestion,
        &tr("Create"),
        move |name| s.input(AppMsg::QuickList(QuickListMsg::QuickListNew(name))),
    );
}

/// Modal text prompt. `on_ok` gets the trimmed text.
fn prompt_name(
    title: &str,
    body: &str,
    initial: &str,
    ok_label: &str,
    on_ok: impl Fn(String) + 'static,
) {
    let parent = gtk::Application::default().active_window();
    let dialog = adw::AlertDialog::new(Some(title), Some(body));
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("ok", ok_label);
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");

    let entry = gtk::Entry::builder()
        .text(initial)
        .activates_default(true)
        .build();
    entry.select_region(0, -1);
    entry.connect_map(|e| {
        e.grab_focus();
    });
    dialog.set_extra_child(Some(&entry));
    dialog.present(parent.as_ref());

    dialog.connect_response(None, move |_, resp| {
        if resp == "ok" {
            on_ok(entry.text().trim().to_string());
        }
    });
}

/// Modal yes/no question whose "yes" is destructive.
fn confirm(title: &str, body: &str, ok_label: &str, on_ok: impl Fn() + 'static) {
    let parent = gtk::Application::default().active_window();
    let dialog = adw::AlertDialog::new(Some(title), Some(body));
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("ok", ok_label);
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.present(parent.as_ref());
    dialog.connect_response(None, move |_, resp| {
        if resp == "ok" {
            on_ok();
        }
    });
}
