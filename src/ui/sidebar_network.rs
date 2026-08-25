//! Sidebar network integration stub.
//!
//! Connected servers are now surfaced directly through `get_system_mounts()`.

use crate::model::AppMsg;
use crate::services::network::NetworkBookmark;
use gtk::gio::prelude::*;
use relm4::Sender;

pub fn build_network_section(_bookmarks: &[NetworkBookmark], _sender: Sender<AppMsg>) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, 0)
}

pub fn connect_volume_monitor_signals(sender: Sender<AppMsg>) {
    let monitor = gtk::gio::VolumeMonitor::get();

    let s = sender.clone();
    monitor.connect_mount_added(move |_, _| {
        let _ = s.send(AppMsg::RefreshSidebar);
    });

    let s = sender.clone();
    monitor.connect_mount_removed(move |_, _| {
        let _ = s.send(AppMsg::RefreshSidebar);
    });

    let s = sender.clone();
    monitor.connect_mount_changed(move |_, _| {
        let _ = s.send(AppMsg::RefreshSidebar);
    });
}
