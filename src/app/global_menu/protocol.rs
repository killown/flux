//! Protocol layer of the global menu: capability detection and the export.
//! Everything is a no-op when the compositor advertises neither appmenu
//! interface, so the app behaves as before there.
//!
//! The menu is served as `com.canonical.dbusmenu` (see [`super::dbusmenu`])
//! and the window is associated with it through `xdg_dbus_annotation_v1`
//! (see [`super::wayland`]). The KDE `org_kde_kwin_appmenu_manager` fallback
//! is detected but not bound yet, so [`bind_surface`] reports
//! [`BindError::ProtocolUnavailable`] for it.

use super::dbusmenu;
use super::wayland;
use adw::gio;
use adw::prelude::*;
use std::cell::RefCell;

/// Registry interface name of the preferred protocol.
const XDG_INTERFACE: &str = "xdg_dbus_annotation_manager_v1";
/// Registry interface name of the KDE fallback.
const KDE_INTERFACE: &str = "org_kde_kwin_appmenu_manager";
/// Parent of every exported object path, the layout other dbusmenu exporters
/// use (`/com/canonical/menu/<n>`).
const PATH_PREFIX: &str = "/com/canonical/menu";

/// Which appmenu protocol the compositor offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMenuCapability {
    Xdg,
    Kde,
    None,
}

/// Why binding a surface failed.
#[derive(Debug)]
pub enum BindError {
    /// Session bus or object registration failed.
    DBus(String),
    /// The Wayland association request could not be sent.
    ProtocolUnavailable,
}

/// Owns the D-Bus export and the Wayland annotation of one window's menu.
pub struct AppMenuBinding {
    capability: AppMenuCapability,
    object_path: String,
    /// Menu served to the panel, shared with the method handlers.
    tree: dbusmenu::Shared,
    export: RefCell<Option<(gio::DBusConnection, gio::RegistrationId)>>,
    annotation: RefCell<Option<wayland::Annotation>>,
}

impl AppMenuBinding {
    /// Binding for `window_id`, the object path is
    /// `/com/canonical/menu/<window_id>`.
    pub fn new(capability: AppMenuCapability, window_id: u64) -> Self {
        Self {
            capability,
            object_path: format!("{PATH_PREFIX}/{window_id}"),
            tree: Default::default(),
            export: RefCell::new(None),
            annotation: RefCell::new(None),
        }
    }

    pub fn object_path(&self) -> &str {
        &self.object_path
    }
}

/// Interface names the compositor advertises.
fn query_registry() -> Vec<String> {
    wayland::registry_interfaces()
}

/// Picks a capability from registry interface names. Xdg wins over Kde.
pub(crate) fn detect_capability_with(interfaces: &[String]) -> AppMenuCapability {
    let has = |name: &str| interfaces.iter().any(|i| i == name);
    if has(XDG_INTERFACE) {
        AppMenuCapability::Xdg
    } else if has(KDE_INTERFACE) {
        AppMenuCapability::Kde
    } else {
        AppMenuCapability::None
    }
}

/// Capability of the running compositor.
pub fn detect_capability() -> AppMenuCapability {
    detect_capability_with(&query_registry())
}

/// Exports `menu` at the binding's object path, then associates it with the
/// surface of `toplevel`. No-op returning `Ok(())` without a capability.
///
/// Uses a synchronous session-bus lookup, so call it from a deferred idle,
/// never from the startup path (invariant I1).
pub fn bind_surface(
    binding: &AppMenuBinding,
    toplevel: &gtk::Window,
    menu: &gio::Menu,
) -> Result<(), BindError> {
    if binding.capability == AppMenuCapability::None {
        return Ok(());
    }
    if binding.capability != AppMenuCapability::Xdg {
        return Err(BindError::ProtocolUnavailable);
    }
    update_bound_menu(binding, menu);
    if binding.export.borrow().is_none() {
        let conn = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
            .map_err(|e| BindError::DBus(e.to_string()))?;
        let id = dbusmenu::register(&conn, &binding.object_path, binding.tree.clone())
            .map_err(|e| BindError::DBus(e.to_string()))?;
        *binding.export.borrow_mut() = Some((conn, id));
    }
    if binding.annotation.borrow().is_none() {
        let service = binding
            .export
            .borrow()
            .as_ref()
            .and_then(|(conn, _)| conn.unique_name())
            .ok_or_else(|| BindError::DBus("no unique bus name".to_string()))?;
        let annotation = wayland::annotate(
            toplevel,
            dbusmenu::INTERFACE,
            &service,
            &binding.object_path,
        )
        .ok_or(BindError::ProtocolUnavailable)?;
        *binding.annotation.borrow_mut() = Some(annotation);
    }
    Ok(())
}

/// Replaces the served menu and signals the change. No-op without a
/// capability.
///
/// `gio::Menu` is not `Send`: build the model data on the blocking pool if
/// needed, but call this on the GTK thread.
pub fn update_bound_menu(binding: &AppMenuBinding, menu: &gio::Menu) {
    if binding.capability == AppMenuCapability::None {
        return;
    }
    let revision = binding
        .tree
        .borrow_mut()
        .replace(menu.upcast_ref::<gio::MenuModel>());
    if let Some((conn, _)) = binding.export.borrow().as_ref() {
        dbusmenu::notify_layout(conn, &binding.object_path, revision);
    }
}

/// Removes the annotation and the D-Bus export. Safe to call twice and on a
/// never-bound binding.
pub fn unbind(binding: &AppMenuBinding) {
    binding.annotation.borrow_mut().take();
    if let Some((conn, id)) = binding.export.borrow_mut().take() {
        let _ = conn.unregister_object(id);
    }
}
