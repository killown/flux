//! `com.canonical.dbusmenu` server: the menu interface global-menu panels
//! read. The tree is rebuilt from a `gio::Menu` on every update and served
//! through `gio`'s object registration, so it adds no D-Bus dependency.
//! Clicking an item activates the same `app.gm-*` action the popover uses.

use adw::gio;
use adw::prelude::*;
use gtk::glib::{self, Variant};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// DBus interface name the compositor annotation advertises.
pub const INTERFACE: &str = "com.canonical.dbusmenu";

/// Disabled placeholder action of the "No selection" item.
const NOOP_ACTION: &str = "app.mb-noop";

const INTROSPECTION: &str = r#"<node>
  <interface name="com.canonical.dbusmenu">
    <method name="GetLayout">
      <arg type="i" name="parentId" direction="in"/>
      <arg type="i" name="recursionDepth" direction="in"/>
      <arg type="as" name="propertyNames" direction="in"/>
      <arg type="u" name="revision" direction="out"/>
      <arg type="(ia{sv}av)" name="layout" direction="out"/>
    </method>
    <method name="GetGroupProperties">
      <arg type="ai" name="ids" direction="in"/>
      <arg type="as" name="propertyNames" direction="in"/>
      <arg type="a(ia{sv})" name="properties" direction="out"/>
    </method>
    <method name="GetProperty">
      <arg type="i" name="id" direction="in"/>
      <arg type="s" name="name" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <method name="Event">
      <arg type="i" name="id" direction="in"/>
      <arg type="s" name="eventId" direction="in"/>
      <arg type="v" name="data" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
    <method name="EventGroup">
      <arg type="a(isvu)" name="events" direction="in"/>
      <arg type="ai" name="idErrors" direction="out"/>
    </method>
    <method name="AboutToShow">
      <arg type="i" name="id" direction="in"/>
      <arg type="b" name="needUpdate" direction="out"/>
    </method>
    <method name="AboutToShowGroup">
      <arg type="ai" name="ids" direction="in"/>
      <arg type="ai" name="updatesNeeded" direction="out"/>
      <arg type="ai" name="idErrors" direction="out"/>
    </method>
    <signal name="ItemsPropertiesUpdated">
      <arg type="a(ia{sv})" name="updatedProps"/>
      <arg type="a(ias)" name="removedProps"/>
    </signal>
    <signal name="LayoutUpdated">
      <arg type="u" name="revision"/>
      <arg type="i" name="parent"/>
    </signal>
    <signal name="ItemActivationRequested">
      <arg type="i" name="id"/>
      <arg type="u" name="timestamp"/>
    </signal>
    <property name="Version" type="u" access="read"/>
    <property name="TextDirection" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="IconThemePath" type="as" access="read"/>
  </interface>
</node>"#;

/// One menu entry. Id 0 is the invisible root.
#[derive(Default)]
struct Item {
    id: i32,
    label: String,
    action: Option<String>,
    enabled: bool,
    submenu: bool,
    /// Target the action is activated with (radio and parameterised entries).
    target: Option<Variant>,
    children: Vec<Item>,
}

/// The served menu and its revision counter.
#[derive(Default)]
pub struct Tree {
    revision: u32,
    root: Item,
}

/// Tree shared between the binding and the D-Bus method handlers.
pub type Shared = Rc<RefCell<Tree>>;

impl Tree {
    /// Replaces the served menu with a flattened copy of `menu` and returns
    /// the new revision. Sections are inlined, submenus are kept.
    pub fn replace(&mut self, menu: &gio::MenuModel) -> u32 {
        let mut next = 0;
        self.root = Item {
            id: 0,
            enabled: true,
            submenu: true,
            children: collect(menu, &mut next),
            ..Item::default()
        };
        self.revision += 1;
        self.revision
    }

    fn find(&self, id: i32) -> Option<&Item> {
        fn walk(item: &Item, id: i32) -> Option<&Item> {
            if item.id == id {
                return Some(item);
            }
            item.children.iter().find_map(|c| walk(c, id))
        }
        walk(&self.root, id)
    }
}

fn string_attr(model: &gio::MenuModel, i: i32, name: &str) -> Option<String> {
    model
        .item_attribute_value(i, name, Some(glib::VariantTy::STRING))
        .and_then(|v| v.get::<String>())
}

/// Whether the entry bound to `action` can be activated right now. Entries
/// without an action (submenus) and actions outside `app.` stay enabled.
fn action_enabled(action: Option<&str>) -> bool {
    let Some(action) = action else { return true };
    if action == NOOP_ACTION {
        return false;
    }
    let Some(name) = action.strip_prefix("app.") else {
        return true;
    };
    gio::Application::default().map_or(true, |app| {
        app.has_action(name) && app.is_action_enabled(name)
    })
}

fn collect(model: &gio::MenuModel, next: &mut i32) -> Vec<Item> {
    let mut out = Vec::new();
    for i in 0..model.n_items() {
        if let Some(section) = model.item_link(i, "section") {
            out.extend(collect(&section, next));
            continue;
        }
        *next += 1;
        let id = *next;
        let action = string_attr(model, i, "action");
        let submenu = model.item_link(i, "submenu");
        out.push(Item {
            id,
            label: string_attr(model, i, "label").unwrap_or_default(),
            enabled: action_enabled(action.as_deref()),
            submenu: submenu.is_some(),
            target: model.item_attribute_value(i, "target", None),
            children: submenu.map(|m| collect(&m, next)).unwrap_or_default(),
            action,
        });
    }
    out
}

fn props(item: &Item) -> HashMap<String, Variant> {
    let mut p = HashMap::new();
    if item.id != 0 {
        // A single `_` marks a mnemonic in dbusmenu, so literal ones are doubled.
        p.insert(
            "label".to_string(),
            item.label.replace('_', "__").to_variant(),
        );
        p.insert("enabled".to_string(), item.enabled.to_variant());
    }
    if item.submenu {
        p.insert("children-display".to_string(), "submenu".to_variant());
    }
    p
}

fn layout(item: &Item, depth: i32) -> Variant {
    let kids: Vec<Variant> = if depth == 0 {
        Vec::new()
    } else {
        item.children
            .iter()
            .map(|c| layout(c, depth - 1).to_variant())
            .collect()
    };
    (item.id, props(item), kids).to_variant()
}

fn flatten<'a>(item: &'a Item, out: &mut Vec<&'a Item>) {
    out.push(item);
    for c in &item.children {
        flatten(c, out);
    }
}

fn activate(tree: &Shared, id: i32) {
    let found = tree
        .borrow()
        .find(id)
        .and_then(|i| i.action.clone().map(|a| (a, i.target.clone())));
    let Some((action, target)) = found else {
        return;
    };
    let name = action.strip_prefix("app.").unwrap_or(&action);
    if let Some(app) = gio::Application::default() {
        app.activate_action(name, target.as_ref());
    }
}

fn int(params: &Variant, i: usize) -> i32 {
    params.child_value(i).get::<i32>().unwrap_or(0)
}

fn call(tree: &Shared, method: &str, params: &Variant, inv: gio::DBusMethodInvocation) {
    match method {
        "GetLayout" => {
            let t = tree.borrow();
            let node = t.find(int(params, 0)).unwrap_or(&t.root);
            let reply =
                Variant::tuple_from_iter([t.revision.to_variant(), layout(node, int(params, 1))]);
            inv.return_value(Some(&reply));
        }
        "GetGroupProperties" => {
            let ids: Vec<i32> = params
                .child_value(0)
                .iter()
                .filter_map(|v| v.get::<i32>())
                .collect();
            let t = tree.borrow();
            let mut all = Vec::new();
            flatten(&t.root, &mut all);
            let out: Vec<(i32, HashMap<String, Variant>)> = all
                .into_iter()
                .filter(|i| ids.is_empty() || ids.contains(&i.id))
                .map(|i| (i.id, props(i)))
                .collect();
            inv.return_value(Some(&(out,).to_variant()));
        }
        "GetProperty" => {
            let name = params.child_value(1).get::<String>().unwrap_or_default();
            let value = tree
                .borrow()
                .find(int(params, 0))
                .and_then(|i| props(i).remove(&name));
            match value {
                Some(v) => inv.return_value(Some(&(v,).to_variant())),
                None => inv.return_dbus_error(
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "no such property",
                ),
            }
        }
        "Event" => {
            if params.child_value(1).get::<String>().as_deref() == Some("clicked") {
                activate(tree, int(params, 0));
            }
            inv.return_value(None);
        }
        "EventGroup" => {
            for ev in params.child_value(0).iter() {
                if ev.child_value(1).get::<String>().as_deref() == Some("clicked") {
                    activate(tree, ev.child_value(0).get::<i32>().unwrap_or(0));
                }
            }
            inv.return_value(Some(&(Vec::<i32>::new(),).to_variant()));
        }
        "AboutToShow" => inv.return_value(Some(&(false,).to_variant())),
        "AboutToShowGroup" => {
            inv.return_value(Some(&(Vec::<i32>::new(), Vec::<i32>::new()).to_variant()));
        }
        _ => inv.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
    }
}

fn property(name: &str) -> Variant {
    match name {
        "Version" => 3u32.to_variant(),
        "TextDirection" => "ltr".to_variant(),
        "Status" => "normal".to_variant(),
        "IconThemePath" => Vec::<String>::new().to_variant(),
        _ => ().to_variant(),
    }
}

/// Registers the dbusmenu object at `path` on `conn`.
pub fn register(
    conn: &gio::DBusConnection,
    path: &str,
    tree: Shared,
) -> Result<gio::RegistrationId, glib::Error> {
    let node = gio::DBusNodeInfo::for_xml(INTROSPECTION)?;
    let iface = node
        .lookup_interface(INTERFACE)
        .ok_or_else(|| glib::Error::new(gio::IOErrorEnum::Failed, "dbusmenu interface missing"))?;
    conn.register_object(path, &iface)
        .method_call(move |_, _, _, _, method, params, inv| call(&tree, method, &params, inv))
        .property(|_, _, _, _, name| property(name))
        .build()
}

/// Tells watchers that the whole layout changed.
pub fn notify_layout(conn: &gio::DBusConnection, path: &str, revision: u32) {
    let _ = conn.emit_signal(
        None::<&str>,
        path,
        INTERFACE,
        "LayoutUpdated",
        Some(&(revision, 0i32).to_variant()),
    );
}
