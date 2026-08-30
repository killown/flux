use crate::model::AppMsg;
use crate::services::network::{self, NetworkBookmark};
use gtk::gio::prelude::*;
use gtk::prelude::*;
use relm4::Sender;

pub fn build_network_section(bookmarks: &[NetworkBookmark], sender: Sender<AppMsg>) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
    container.set_hexpand(false);
    container.set_halign(gtk::Align::Start);

    let add_row = |name: &str, icon_name: &str, uri: &str, is_mount: bool| {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        row.add_css_class("sidebar-row");
        if is_mount {
            row.add_css_class("sidebar-mount");
        }
        row.set_hexpand(false);
        row.set_halign(gtk::Align::Start);

        let img = gtk::Image::from_icon_name(icon_name);
        let lbl = gtk::Label::builder()
            .label(name)
            .halign(gtk::Align::Start)
            .hexpand(false)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        row.append(&img);
        row.append(&lbl);

        if is_mount {
            let eject_btn = gtk::Button::builder()
                .icon_name("media-eject")
                .valign(gtk::Align::Center)
                .css_classes(["eject-button"])
                .build();

            let uri_unmount = uri.to_string();
            let s_unmount = sender.clone();
            eject_btn.connect_clicked(move |_| {
                let _ = s_unmount.send(AppMsg::UnmountNetwork(uri_unmount.clone()));
            });
            row.append(&eject_btn);
        }

        let uri_nav = uri.to_string();
        let s_nav = sender.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |gesture, _, _, _| {
            if gesture.current_button() == 1 {
                let _ = s_nav.send(AppMsg::Navigate(std::path::PathBuf::from(&uri_nav)));
            }
        });
        row.add_controller(click);
        container.append(&row);
    };

    for bookmark in bookmarks {
        add_row(&bookmark.name, &bookmark.icon, &bookmark.uri, false);
    }

    let active = network::active_mounts();
    if !active.is_empty() {
        let label = gtk::Label::builder()
            .label(crate::i18n::tr("Connected Servers"))
            .halign(gtk::Align::Start)
            .margin_start(2)
            .margin_top(6)
            .margin_bottom(2)
            .css_classes(["sidebar-section-label"])
            .build();
        container.append(&label);

        for (uri, name, icon) in active {
            add_row(&name, &icon, &uri, true);
        }
    }

    container
}

pub fn connect_volume_monitor_signals(sender: Sender<AppMsg>) {
    let monitor = gtk::gio::VolumeMonitor::get();

    let s = sender.clone();
    monitor.connect_mount_added(move |_, _| {
        let _ = s.send(AppMsg::RefreshNetworkSidebar);
    });

    let s = sender.clone();
    monitor.connect_mount_removed(move |_, _| {
        let _ = s.send(AppMsg::RefreshNetworkSidebar);
    });

    let s = sender.clone();
    monitor.connect_mount_changed(move |_, _| {
        let _ = s.send(AppMsg::RefreshNetworkSidebar);
    });
}
