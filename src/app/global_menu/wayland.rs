//! Wayland side of the global menu: registry query and the
//! `xdg_dbus_annotation_v1` association of a window with a DBus object.
//!
//! GTK owns the Wayland connection, so this module wraps GTK's `wl_display`
//! and `wl_surface` instead of opening a second connection. Nothing here
//! blocks beyond one registry roundtrip, and callers run it from a deferred
//! timer (invariant I1).

use gtk::prelude::*;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

#[allow(clippy::all, non_camel_case_types, unused_imports, dead_code)]
mod proto {
    use wayland_client::protocol::*;
    use wayland_client::{self};

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/xdg-dbus-annotation-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/xdg-dbus-annotation-v1.xml");
}

use proto::xdg_dbus_annotation_manager_v1::XdgDbusAnnotationManagerV1;
use proto::xdg_dbus_annotation_v1::XdgDbusAnnotationV1;

/// Dispatch target, the protocol has no events, so it carries no state.
struct App;

impl Dispatch<WlRegistry, GlobalListContents> for App {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgDbusAnnotationManagerV1, ()> for App {
    fn event(
        _: &mut Self,
        _: &XdgDbusAnnotationManagerV1,
        _: proto::xdg_dbus_annotation_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgDbusAnnotationV1, ()> for App {
    fn event(
        _: &mut Self,
        _: &XdgDbusAnnotationV1,
        _: proto::xdg_dbus_annotation_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// Wraps GTK's own `wl_display`. `None` on X11 or without a display.
fn connection() -> Option<Connection> {
    let display = gtk::gdk::Display::default()?;
    if !display.is::<gdk4_wayland::WaylandDisplay>() {
        return None;
    }
    let ptr =
        unsafe { gdk4_wayland::ffi::gdk_wayland_display_get_wl_display(display.as_ptr().cast()) };
    if ptr.is_null() {
        return None;
    }
    let backend = unsafe { Backend::from_foreign_display(ptr.cast()) };
    Some(Connection::from_backend(backend))
}

/// Interface names of every global the compositor advertises.
pub fn registry_interfaces() -> Vec<String> {
    let Some(conn) = connection() else {
        return Vec::new();
    };
    let Ok((globals, _queue)) = registry_queue_init::<App>(&conn) else {
        return Vec::new();
    };
    globals
        .contents()
        .with_list(|list| list.iter().map(|g| g.interface.clone()).collect())
}

/// A live annotation, dropping it tells the compositor to forget the object.
pub struct Annotation {
    conn: Connection,
    annotation: XdgDbusAnnotationV1,
}

impl Drop for Annotation {
    fn drop(&mut self) {
        self.annotation.destroy();
        let _ = self.conn.flush();
    }
}

/// Associates `toplevel` with the DBus object `service`/`path`, which must
/// already be exported and implement `interface`. `None` when the window has
/// no Wayland surface or the compositor lacks the protocol.
pub fn annotate(
    toplevel: &gtk::Window,
    interface: &str,
    service: &str,
    path: &str,
) -> Option<Annotation> {
    let surface = toplevel.surface()?;
    if !surface.is::<gdk4_wayland::WaylandSurface>() {
        return None;
    }
    let conn = connection()?;
    let sptr =
        unsafe { gdk4_wayland::ffi::gdk_wayland_surface_get_wl_surface(surface.as_ptr().cast()) };
    if sptr.is_null() {
        return None;
    }
    let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), sptr.cast()) }.ok()?;
    let wl_surface = WlSurface::from_id(&conn, id).ok()?;

    let (globals, mut queue) = registry_queue_init::<App>(&conn).ok()?;
    let qh = queue.handle();
    let manager: XdgDbusAnnotationManagerV1 = globals.bind(&qh, 1..=1, ()).ok()?;
    let annotation = manager.create_surface(interface.to_string(), &wl_surface, &qh, ());
    annotation.set_address(service.to_string(), path.to_string());
    manager.destroy();
    let _ = queue.roundtrip(&mut App);
    Some(Annotation { conn, annotation })
}
