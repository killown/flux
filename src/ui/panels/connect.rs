use super::header::panel_header;
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::services::network::ConnectToServerParams;
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::rc::Rc;

struct ProtocolEntry {
    label: &'static str,
    scheme: &'static str,
    default_port: Option<u16>,
}

const PROTOCOLS: &[ProtocolEntry] = &[
    ProtocolEntry {
        label: "Windows Share (SMB/Samba)",
        scheme: "smb",
        default_port: None,
    },
    ProtocolEntry {
        label: "SSH / SFTP",
        scheme: "sftp",
        default_port: Some(22),
    },
    ProtocolEntry {
        label: "WebDAV (HTTP)",
        scheme: "dav",
        default_port: Some(80),
    },
    ProtocolEntry {
        label: "WebDAV (HTTPS)",
        scheme: "davs",
        default_port: Some(443),
    },
    ProtocolEntry {
        label: "NFS",
        scheme: "nfs",
        default_port: None,
    },
    ProtocolEntry {
        label: "FTP",
        scheme: "ftp",
        default_port: Some(21),
    },
    ProtocolEntry {
        label: "FTP (TLS)",
        scheme: "ftps",
        default_port: Some(990),
    },
    ProtocolEntry {
        label: "AFP (Apple Filing)",
        scheme: "afp",
        default_port: Some(548),
    },
];

/// Adds a row with a small caption above an entry and returns the entry.
fn stacked_entry(
    group: &adw::PreferencesGroup,
    title: &str,
    placeholder: &str,
    purpose: gtk::InputPurpose,
) -> gtk::Entry {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_start(10)
        .margin_end(10)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let label = gtk::Label::builder()
        .label(title)
        .halign(gtk::Align::Start)
        .css_classes(["caption", "dim-label"])
        .build();

    let entry = gtk::Entry::builder()
        .placeholder_text(placeholder)
        .input_purpose(purpose)
        .hexpand(true)
        .build();

    container.append(&label);
    container.append(&entry);

    let row = adw::PreferencesRow::builder().child(&container).build();
    group.add(&row);
    entry
}

/// Builds the connect-to-server panel.
pub fn build_connect_panel(sender: AsyncComponentSender<FluxApp>) -> gtk::Box {
    let spec = PanelSpec::connect();

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    let connect_btn = gtk::Button::builder()
        .label(tr("Connect"))
        .css_classes(["suggested-action", "pill"])
        .valign(gtk::Align::Center)
        .build();

    {
        let s = sender.clone();
        let header = panel_header(
            Some("network-server-symbolic"),
            &tr("Connect to Server"),
            &[connect_btn.clone().upcast::<gtk::Widget>()],
            move || s.input(AppMsg::ToggleConnectPanel),
        );
        panel.append(&header);
    }

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(12)
        .build();

    let group = adw::PreferencesGroup::new();

    let protocol_row = adw::ComboRow::new();
    protocol_row.set_title(&tr("Protocol"));
    let labels: Vec<&str> = PROTOCOLS.iter().map(|p| p.label).collect();
    protocol_row.set_model(Some(&gtk::StringList::new(&labels)));
    group.add(&protocol_row);

    let host_entry = stacked_entry(
        &group,
        &tr("Server Address"),
        "server.example.com",
        gtk::InputPurpose::Url,
    );
    let port_entry = stacked_entry(
        &group,
        &tr("Port"),
        &tr("optional"),
        gtk::InputPurpose::Digits,
    );
    let path_entry = stacked_entry(
        &group,
        &tr("Share / Path"),
        &tr("optional"),
        gtk::InputPurpose::FreeForm,
    );
    let user_entry = stacked_entry(
        &group,
        &tr("Username"),
        &tr("optional"),
        gtk::InputPurpose::FreeForm,
    );

    content_box.append(&group);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(false)
        .hexpand(false)
        .vexpand(true)
        .child(&content_box)
        .build();
    panel.append(&scrolled);

    // Selecting a protocol fills in its default port.
    {
        let port_entry = port_entry.clone();
        protocol_row.connect_selected_notify(move |row| {
            if let Some(entry) = PROTOCOLS.get(row.selected() as usize) {
                port_entry.set_text(
                    &entry
                        .default_port
                        .map(|p| p.to_string())
                        .unwrap_or_default(),
                );
            }
        });
    }

    // Clears the error highlight once the user edits the host.
    {
        host_entry.connect_changed(|e| e.remove_css_class("error"));
    }

    // Builds the server URI from the form, then closes the panel and connects.
    let submit: Rc<dyn Fn()> = {
        let host_entry = host_entry.clone();
        let port_entry = port_entry.clone();
        let path_entry = path_entry.clone();
        let user_entry = user_entry.clone();
        let protocol_row = protocol_row.clone();
        let s = sender.clone();
        Rc::new(move || {
            let host = host_entry.text().trim().to_owned();
            if host.is_empty() {
                host_entry.add_css_class("error");
                host_entry.grab_focus();
                return;
            }

            let non_empty = |e: &gtk::Entry| {
                let t = e.text().trim().to_owned();
                (!t.is_empty()).then_some(t)
            };

            let protocol = PROTOCOLS
                .get(protocol_row.selected() as usize)
                .map(|p| p.scheme)
                .unwrap_or("smb")
                .to_owned();

            let params = ConnectToServerParams {
                protocol,
                host,
                port: port_entry.text().trim().parse().ok().filter(|&p| p > 0),
                path: non_empty(&path_entry),
                username: non_empty(&user_entry),
            };

            if let Some(uri) = params.build_uri() {
                s.input(AppMsg::ToggleConnectPanel);
                s.input(AppMsg::ConnectToServer {
                    uri,
                    credentials: None,
                });
            }
        })
    };

    {
        let submit = submit.clone();
        connect_btn.connect_clicked(move |_| submit());
    }
    for entry in [&host_entry, &port_entry, &path_entry, &user_entry] {
        let submit = submit.clone();
        entry.connect_activate(move |_| submit());
    }

    // Each time the panel is shown, focus the host entry.
    {
        let host_entry = host_entry.clone();
        panel.connect_map(move |_| {
            host_entry.grab_focus();
        });
    }

    let root = resizable_panel(&spec, &panel, |_| {});

    let esc_ctrl = gtk::EventControllerKey::new();
    esc_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let s = sender.clone();
        esc_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Escape {
                s.input(AppMsg::ToggleConnectPanel);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
    }
    root.add_controller(esc_ctrl);

    root
}
