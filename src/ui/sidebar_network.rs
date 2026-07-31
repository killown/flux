//! Sidebar "Connected Servers" section builder.
//!
//! Renders a single dynamic sidebar group:
//!
//! **Connected Servers** - Live GVFS mounts surfaced by
//! [`crate::services::network::active_mounts`], each with an unmount button.
//! Hidden completely when no shares are mounted.
//!
//! Rebuilt on every [`crate::model::AppMsg::RefreshSidebar`] and
//! [`crate::model::AppMsg::RefreshNetworkSidebar`] dispatch, keeping it in sync
//! with runtime GVFS state changes (mount/unmount events from `VolumeMonitor`).

use adw::prelude::*;
use relm4::Sender;

use crate::model::AppMsg;
use crate::services::network::{active_mounts, NetworkBookmark};

/// Builds and returns the "Connected Servers" sidebar section widget.
///
/// The returned `gtk::Box` is meant to be appended directly to the sidebar
/// container. It remains hidden/empty when no network shares are mounted,
/// and dynamically populates active GVFS network mounts.
pub fn build_network_section(_bookmarks: &[NetworkBookmark], sender: Sender<AppMsg>) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // ── Connected servers sub-section ─────────────────────────────────────────

    let mounts = active_mounts();

    if !mounts.is_empty() {
        let mounts_label = gtk::Label::new(Some(&crate::i18n::tr("Connected Servers")));
        mounts_label.set_halign(gtk::Align::Start);
        mounts_label.set_margin_start(12);
        mounts_label.set_margin_top(8);
        mounts_label.set_margin_bottom(2);
        mounts_label.add_css_class("dim-label");
        mounts_label.add_css_class("caption");
        mounts_label.add_css_class("sidebar-section");
        section.append(&mounts_label);

        let mounts_list = gtk::ListBox::new();
        mounts_list.add_css_class("navigation-sidebar");
        mounts_list.set_selection_mode(gtk::SelectionMode::Single);

        for (uri, name, icon) in mounts {
            let row = make_mount_row(&icon, &name, &uri, sender.clone());
            mounts_list.append(&row);
        }

        section.append(&mounts_list);
    }

    section
}

// ─── Row builders ─────────────────────────────────────────────────────────────

/// Creates a sidebar row for an active GVFS mount with an eject/unmount button.
fn make_mount_row(icon: &str, label: &str, uri: &str, sender: Sender<AppMsg>) -> gtk::ListBoxRow {
    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);
    row_box.set_margin_top(4);
    row_box.set_margin_bottom(4);

    let img = gtk::Image::from_icon_name(icon);
    img.set_icon_size(gtk::IconSize::Normal);
    row_box.append(&img);

    let lbl = gtk::Label::new(Some(label));
    lbl.set_halign(gtk::Align::Start);
    lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    lbl.set_hexpand(true);
    row_box.append(&lbl);

    let eject_btn = gtk::Button::from_icon_name("media-eject-symbolic");
    eject_btn.add_css_class("flat");
    eject_btn.add_css_class("circular");
    eject_btn.set_tooltip_text(Some(&crate::i18n::tr("Disconnect")));

    let uri_eject = uri.to_owned();
    let sender_eject = sender.clone();
    eject_btn.connect_clicked(move |_| {
        let _ = sender_eject.send(AppMsg::UnmountNetwork(uri_eject.clone()));
    });
    row_box.append(&eject_btn);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&row_box));

    // Use GestureClick to reliably capture navigation clicks on active mount rows
    let uri_nav = uri.to_owned();
    let sender_nav = sender.clone();
    let click = gtk::GestureClick::new();
    click.connect_released(move |_, _, _, _| {
        let _ = sender_nav.send(AppMsg::Navigate(std::path::PathBuf::from(&uri_nav)));
    });
    row_box.add_controller(click);

    row
}

// ─── VolumeMonitor integration ────────────────────────────────────────────────

/// Connects GVFS `VolumeMonitor` signals so the sidebar auto-updates on
/// mount/unmount events without requiring a full sidebar rebuild poll.
///
/// Call once from `ui/init.rs` after the initial sidebar is built.
///
/// # Arguments
/// * `sender` - Receives [`AppMsg::RefreshNetworkSidebar`] on every mount change.
pub fn connect_volume_monitor_signals(sender: Sender<AppMsg>) {
    let monitor = gtk::gio::VolumeMonitor::get();
    eprintln!(
        "[DEBUG volume_monitor] connect_volume_monitor_signals attached to VolumeMonitor instance."
    );

    {
        let s = sender.clone();
        monitor.connect_mount_added(move |_, mount| {
            let name = mount.name().to_string();
            let uri = mount.root().uri().to_string();
            eprintln!(
                "[DEBUG volume_monitor] SIGNAL: mount_added triggered! Name='{}', URI='{}'",
                name, uri
            );
            let _ = s.send(AppMsg::RefreshNetworkSidebar);
        });
    }
    {
        let s = sender.clone();
        monitor.connect_mount_removed(move |_, mount| {
            let name = mount.name().to_string();
            let uri = mount.root().uri().to_string();
            eprintln!(
                "[DEBUG volume_monitor] SIGNAL: mount_removed triggered! Name='{}', URI='{}'",
                name, uri
            );
            let _ = s.send(AppMsg::RefreshNetworkSidebar);
        });
    }
    {
        let s = sender.clone();
        monitor.connect_mount_changed(move |_, mount| {
            let name = mount.name().to_string();
            let uri = mount.root().uri().to_string();
            eprintln!(
                "[DEBUG volume_monitor] SIGNAL: mount_changed triggered! Name='{}', URI='{}'",
                name, uri
            );
            let _ = s.send(AppMsg::RefreshNetworkSidebar);
        });
    }
}
