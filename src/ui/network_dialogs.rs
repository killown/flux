//! GTK4/Libadwaita dialogs for network connection and credential entry.
//!
//! Provides two dialogs:
//! - [`show_connect_to_server`] - collects protocol, host, port, path, and username,
//!   then dispatches [`crate::model::AppMsg::ConnectToServer`].
//! - [`show_credentials_dialog`] - prompts for a username and password when GVFS
//!   reports that a remote location requires authentication.
//!
//! Both dialogs are non-blocking modal windows that dispatch an [`AppMsg`] on
//! confirmation, keeping the GTK event loop free.

use adw::prelude::*;
use relm4::Sender;

use crate::model::AppMsg;
use crate::services::network::{ConnectToServerParams, NetworkAuthFlags, NetworkCredentials};

// ─── Protocol metadata ────────────────────────────────────────────────────────

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

// ─── "Connect to Server" dialog ───────────────────────────────────────────────

/// Presents a beautiful, modal dialog to connect to a remote server.
///
/// Dispatches [`AppMsg::ConnectToServer`] on confirmation.
pub fn show_connect_to_server(parent: &impl IsA<gtk::Window>, sender: Sender<AppMsg>) {
    let window = adw::Window::new();
    window.set_title(Some(&crate::i18n::tr("Connect to Server")));
    window.set_modal(true);
    window.set_transient_for(Some(parent));
    window.set_default_size(440, -1);
    window.set_resizable(false);

    // ── Main layout ──────────────────────────────────────────────────────────

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // ── Header bar ────────────────────────────────────────────────────────────

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some(&crate::i18n::tr(
        "Connect to Server",
    )))));

    let cancel_btn = gtk::Button::with_label(&crate::i18n::tr("Cancel"));
    cancel_btn.add_css_class("flat");
    header.pack_start(&cancel_btn);

    let connect_btn = gtk::Button::with_label(&crate::i18n::tr("Connect"));
    connect_btn.add_css_class("suggested-action");
    header.pack_end(&connect_btn);

    main_box.append(&header);

    // ── Content ──────────────────────────────────────────────────────────────

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    // ── Form fields ──────────────────────────────────────────────────────────

    // Compact list box instead of PreferencesGroup for tighter height control
    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();

    // Protocol
    let protocol_row = adw::ComboRow::new();
    protocol_row.set_title(&crate::i18n::tr("Protocol"));
    let protocol_strings: Vec<&str> = PROTOCOLS.iter().map(|p| p.label).collect();
    let protocol_model = gtk::StringList::new(&protocol_strings);
    protocol_row.set_model(Some(&protocol_model));
    list_box.append(&protocol_row);

    // Host / Server
    let host_entry = gtk::Entry::builder()
        .input_purpose(gtk::InputPurpose::Url)
        .placeholder_text("server.example.com")
        .valign(gtk::Align::Center)
        .build();
    let host_row = adw::ActionRow::builder()
        .title(crate::i18n::tr("Server Address"))
        .activatable_widget(&host_entry)
        .build();
    host_row.add_suffix(&host_entry);
    list_box.append(&host_row);

    // Port
    let port_entry = gtk::Entry::builder()
        .input_purpose(gtk::InputPurpose::Digits)
        .placeholder_text(crate::i18n::tr("optional"))
        .valign(gtk::Align::Center)
        .build();
    let port_row = adw::ActionRow::builder()
        .title(crate::i18n::tr("Port"))
        .activatable_widget(&port_entry)
        .build();
    port_row.add_suffix(&port_entry);
    list_box.append(&port_row);

    // Path / Share
    let path_entry = gtk::Entry::builder()
        .placeholder_text(crate::i18n::tr("optional"))
        .valign(gtk::Align::Center)
        .build();
    let path_row = adw::ActionRow::builder()
        .title(crate::i18n::tr("Share / Path"))
        .activatable_widget(&path_entry)
        .build();
    path_row.add_suffix(&path_entry);
    list_box.append(&path_row);

    // Username
    let user_entry = gtk::Entry::builder()
        .placeholder_text(crate::i18n::tr("optional"))
        .valign(gtk::Align::Center)
        .build();
    let user_row = adw::ActionRow::builder()
        .title(crate::i18n::tr("Username"))
        .activatable_widget(&user_entry)
        .build();
    user_row.add_suffix(&user_entry);
    list_box.append(&user_row);

    content.append(&list_box);
    main_box.append(&content);

    window.set_content(Some(&main_box));

    // ── Protocol change → update default port ──────────────────────────────

    protocol_row.connect_selected_notify({
        let port_entry = port_entry.clone();
        move |row| {
            let idx = row.selected() as usize;
            if let Some(entry) = PROTOCOLS.get(idx) {
                if let Some(port) = entry.default_port {
                    port_entry.set_text(&port.to_string());
                } else {
                    port_entry.set_text("");
                }
            }
        }
    });

    // ── Buttons behaviour ────────────────────────────────────────────────────

    let window_clone = window.clone();
    cancel_btn.connect_clicked(move |_| window_clone.close());

    let window_clone2 = window.clone();
    connect_btn.connect_clicked({
        let host_entry = host_entry.clone();
        let port_entry = port_entry.clone();
        let path_entry = path_entry.clone();
        let user_entry = user_entry.clone();
        let protocol_row = protocol_row.clone();
        let sender = sender.clone();

        move |_| {
            let idx = protocol_row.selected() as usize;
            let scheme = PROTOCOLS
                .get(idx)
                .map(|p| p.scheme)
                .unwrap_or("smb")
                .to_owned();

            let host = host_entry.text().trim().to_owned();
            if host.is_empty() {
                host_entry.add_css_class("error");
                return;
            }
            host_entry.remove_css_class("error");

            let port: Option<u16> = port_entry.text().trim().parse().ok().filter(|&p| p > 0);

            let path = {
                let p = path_entry.text().trim().to_owned();
                if p.is_empty() {
                    None
                } else {
                    Some(p)
                }
            };

            let username = {
                let u = user_entry.text().trim().to_owned();
                if u.is_empty() {
                    None
                } else {
                    Some(u)
                }
            };

            let params = ConnectToServerParams {
                protocol: scheme,
                host,
                port,
                path,
                username,
            };

            if let Some(uri) = params.build_uri() {
                let _ = sender.send(AppMsg::ConnectToServer {
                    uri,
                    credentials: None,
                });
            }
            window_clone2.close();
        }
    });

    // Focus the host entry on show.
    window.connect_show({
        let host_entry = host_entry.clone();
        move |_| {
            host_entry.grab_focus();
        }
    });

    window.present();
}

// ─── Credentials dialog ───────────────────────────────────────────────────────

/// Presents a modal dialog for entering credentials for a network location.
pub fn show_credentials_dialog(
    parent: &impl IsA<gtk::Window>,
    uri: String,
    message: String,
    flags: NetworkAuthFlags,
    auth_failed: bool,
    sender: Sender<AppMsg>,
) {
    let window = adw::Window::new();
    window.set_title(Some(&crate::i18n::tr("Authentication Required")));
    window.set_modal(true);
    window.set_transient_for(Some(parent));
    window.set_default_size(480, -1);
    window.set_resizable(false);

    // ── Main layout ──────────────────────────────────────────────────────────

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // ── Header bar ────────────────────────────────────────────────────────────

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some(&crate::i18n::tr(
        "Authentication Required",
    )))));

    let cancel_btn = gtk::Button::with_label(&crate::i18n::tr("Cancel"));
    cancel_btn.add_css_class("flat");
    header.pack_start(&cancel_btn);

    let connect_btn = gtk::Button::with_label(&crate::i18n::tr("Connect"));
    connect_btn.add_css_class("suggested-action");
    header.pack_end(&connect_btn);

    main_box.append(&header);

    // ── Content ──────────────────────────────────────────────────────────────

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(600);
    clamp.set_child(Some(&content));

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_height(true)
        .build();
    scroller.set_child(Some(&clamp));

    main_box.append(&scroller);

    window.set_content(Some(&main_box));

    // ── Error banner ──────────────────────────────────────────────────────────

    if auth_failed {
        let banner = gtk::Label::new(Some(&crate::i18n::tr(
            "Incorrect credentials. Please try again.",
        )));
        banner.set_wrap(true);
        banner.add_css_class("error");
        banner.set_margin_bottom(12);
        content.append(&banner);
    }

    // ── Description ───────────────────────────────────────────────────────────

    let desc = gtk::Label::new(Some(&message));
    desc.set_wrap(true);
    desc.set_halign(gtk::Align::Start);
    desc.add_css_class("dim-label");
    desc.set_margin_bottom(12);
    content.append(&desc);

    // ── Form fields ──────────────────────────────────────────────────────────

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    page.add(&group);

    let mut username_entry: Option<gtk::Entry> = None;
    let mut password_entry: Option<gtk::PasswordEntry> = None;
    let mut domain_entry: Option<gtk::Entry> = None;
    let mut anon_switch: Option<gtk::Switch> = None;

    if flags.contains(NetworkAuthFlags::ANON_OK) {
        let sw = gtk::Switch::new();
        anon_switch = Some(sw.clone());
        let row = adw::ActionRow::builder()
            .title(crate::i18n::tr("Connect anonymously"))
            .activatable_widget(&sw)
            .build();
        row.add_suffix(&sw);
        group.add(&row);
    }

    if flags.contains(NetworkAuthFlags::USERNAME) {
        let entry = gtk::Entry::builder()
            .placeholder_text(crate::i18n::tr("Username"))
            .build();
        username_entry = Some(entry.clone());
        let row = adw::ActionRow::builder()
            .title(crate::i18n::tr("Username"))
            .activatable_widget(&entry)
            .build();
        row.add_suffix(&entry);
        group.add(&row);
    }

    if flags.contains(NetworkAuthFlags::DOMAIN) {
        let entry = gtk::Entry::builder()
            .placeholder_text(crate::i18n::tr("Domain"))
            .build();
        domain_entry = Some(entry.clone());
        let row = adw::ActionRow::builder()
            .title(crate::i18n::tr("Domain"))
            .activatable_widget(&entry)
            .build();
        row.add_suffix(&entry);
        group.add(&row);
    }

    if flags.contains(NetworkAuthFlags::PASSWORD) {
        let entry = gtk::PasswordEntry::builder()
            .placeholder_text(crate::i18n::tr("Password"))
            .show_peek_icon(true)
            .build();
        password_entry = Some(entry.clone());
        let row = adw::ActionRow::builder()
            .title(crate::i18n::tr("Password"))
            .activatable_widget(&entry)
            .build();
        row.add_suffix(&entry);
        group.add(&row);
    }

    if let Some(sw) = &anon_switch {
        let username_entry = username_entry.clone();
        let domain_entry = domain_entry.clone();
        let password_entry = password_entry.clone();
        sw.connect_active_notify(move |sw| {
            let is_anon = sw.is_active();
            if let Some(e) = &username_entry {
                e.set_sensitive(!is_anon);
            }
            if let Some(e) = &domain_entry {
                e.set_sensitive(!is_anon);
            }
            if let Some(e) = &password_entry {
                e.set_sensitive(!is_anon);
            }
        });
    }

    content.append(&page);

    // ── Buttons behaviour ────────────────────────────────────────────────────

    let window_clone = window.clone();
    cancel_btn.connect_clicked(move |_| window_clone.close());

    let window_clone2 = window.clone();
    connect_btn.connect_clicked({
        let uri = uri.clone();
        let sender = sender.clone();
        let username_entry = username_entry.clone();
        let password_entry = password_entry.clone();
        let domain_entry = domain_entry.clone();
        let anon_switch = anon_switch.clone();

        move |_| {
            let anonymous = anon_switch.as_ref().map(|s| s.is_active()).unwrap_or(false);

            let credentials = if anonymous {
                NetworkCredentials::anonymous()
            } else {
                let username = username_entry
                    .as_ref()
                    .map(|e| e.text().trim().to_owned())
                    .filter(|s| !s.is_empty());
                let password = password_entry
                    .as_ref()
                    .map(|e| e.text().trim().to_owned())
                    .filter(|s| !s.is_empty());
                let domain = domain_entry
                    .as_ref()
                    .map(|e| e.text().trim().to_owned())
                    .filter(|s| !s.is_empty());

                NetworkCredentials {
                    username,
                    password,
                    domain,
                    anonymous: false,
                }
            };

            let _ = sender.send(AppMsg::ConnectToServer {
                uri: uri.clone(),
                credentials: Some(credentials),
            });
            window_clone2.close();
        }
    });

    // Focus the first entry.
    window.connect_show({
        let username_entry = username_entry.clone();
        let password_entry = password_entry.clone();
        move |_| {
            if let Some(e) = &username_entry {
                e.grab_focus();
            } else if let Some(e) = &password_entry {
                e.grab_focus();
            }
        }
    });

    window.present();
}

// ─── "Connect to Server" menu shortcut helper ─────────────────────────────────

pub fn register_connect_action(
    app: &adw::Application,
    window: &adw::Window,
    sender: Sender<AppMsg>,
) {
    use gtk::gio;

    let action = gio::SimpleAction::new("connect-to-server", None);
    let window_c = window.clone();
    let sender_c = sender.clone();

    action.connect_activate(move |_, _| {
        show_connect_to_server(&window_c, sender_c.clone());
    });

    app.add_action(&action);
    app.set_accels_for_action("app.connect-to-server", &["<Primary><Shift>l"]);
}
