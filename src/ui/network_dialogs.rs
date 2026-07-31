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
use gtk::glib;
use relm4::Sender;

use crate::model::AppMsg;
use crate::services::network::{ConnectToServerParams, NetworkAuthFlags};

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

/// Presents the "Connect to Server" dialog and dispatches [`AppMsg::ConnectToServer`]
/// on confirmation.
///
/// The dialog is destroyed after the user clicks "Connect" or "Cancel".
/// All field values are cloned into the closure before the dialog is dismissed.
///
/// # Arguments
/// * `parent`  - The window to use as the transient parent for modal placement.
/// * `sender`  - Application sender, receives [`AppMsg::ConnectToServer`] on success.
pub fn show_connect_to_server(parent: &impl IsA<gtk::Window>, sender: Sender<AppMsg>) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&crate::i18n::tr("Connect to Server"));
    dialog.set_can_close(true);
    dialog.set_default_widget(gtk::Widget::NONE);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_spacing(12);
    content.set_width_request(400);

    // ── Protocol chooser ──────────────────────────────────────────────────────

    let protocol_label = gtk::Label::new(Some(&crate::i18n::tr("Protocol")));
    protocol_label.set_halign(gtk::Align::Start);
    let protocol_row = adw::ComboRow::new();
    protocol_row.set_title(&crate::i18n::tr("Protocol"));
    let protocol_strings: Vec<&str> = PROTOCOLS.iter().map(|p| p.label).collect();
    let protocol_model = gtk::StringList::new(&protocol_strings);
    protocol_row.set_model(Some(&protocol_model));

    // ── Host / Server field ───────────────────────────────────────────────────

    let host_row = adw::EntryRow::new();
    host_row.set_title(&crate::i18n::tr("Server Address"));
    host_row.set_input_purpose(gtk::InputPurpose::Url);

    // ── Port field (optional) ─────────────────────────────────────────────────

    let port_row = adw::EntryRow::new();
    port_row.set_title(&crate::i18n::tr("Port (optional)"));
    port_row.set_input_purpose(gtk::InputPurpose::Digits);

    // ── Path / Share field ────────────────────────────────────────────────────

    let path_row = adw::EntryRow::new();
    path_row.set_title(&crate::i18n::tr("Share / Path (optional)"));

    // ── Username field ────────────────────────────────────────────────────────

    let user_row = adw::EntryRow::new();
    user_row.set_title(&crate::i18n::tr("Username (optional)"));

    // ── Grouped preferences page ──────────────────────────────────────────────

    let prefs_group = adw::PreferencesGroup::new();
    prefs_group.add(&host_row);
    prefs_group.add(&port_row);
    prefs_group.add(&path_row);
    prefs_group.add(&user_row);

    // ── Buttons ───────────────────────────────────────────────────────────────

    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    button_box.set_halign(gtk::Align::End);

    let cancel_btn = gtk::Button::with_label(&crate::i18n::tr("Cancel"));
    cancel_btn.add_css_class("pill");

    let connect_btn = gtk::Button::with_label(&crate::i18n::tr("Connect"));
    connect_btn.add_css_class("pill");
    connect_btn.add_css_class("suggested-action");

    button_box.append(&cancel_btn);
    button_box.append(&connect_btn);

    content.append(&protocol_row);
    content.append(&prefs_group);
    content.append(&button_box);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));

    // ── Protocol change → update default port ─────────────────────────────────

    let port_row_clone = port_row.clone();
    protocol_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        if let Some(entry) = PROTOCOLS.get(idx) {
            if let Some(port) = entry.default_port {
                port_row_clone.set_text(&port.to_string());
            } else {
                port_row_clone.set_text("");
            }
        }
    });

    // ── Cancel ───────────────────────────────────────────────────────────────

    let dialog_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_cancel.close();
    });

    // ── Connect ───────────────────────────────────────────────────────────────

    let dialog_connect = dialog.clone();
    let host_row_c = host_row.clone();
    let port_row_c = port_row.clone();
    let path_row_c = path_row.clone();
    let user_row_c = user_row.clone();
    let protocol_row_c = protocol_row.clone();

    connect_btn.connect_clicked(move |_| {
        let idx = protocol_row_c.selected() as usize;
        let scheme = PROTOCOLS
            .get(idx)
            .map(|p| p.scheme)
            .unwrap_or("smb")
            .to_owned();

        let host = host_row_c.text().trim().to_owned();
        if host.is_empty() {
            host_row_c.add_css_class("error");
            return;
        }
        host_row_c.remove_css_class("error");

        let port: Option<u16> = port_row_c.text().trim().parse().ok().filter(|&p| p > 0);

        let path = {
            let p = path_row_c.text().trim().to_owned();
            if p.is_empty() {
                None
            } else {
                Some(p)
            }
        };

        let username = {
            let u = user_row_c.text().trim().to_owned();
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

        dialog_connect.close();
    });

    dialog.present(parent);
}

// ─── Credentials dialog ───────────────────────────────────────────────────────

/// Presents a credentials dialog for the given network URI and dispatches
/// [`AppMsg::ConnectToServer`] with the filled credentials on confirmation.
///
/// Shows a domain field when `flags` contains [`NetworkAuthFlags::DOMAIN`].
/// Shows an "anonymous" toggle when `flags` contains [`NetworkAuthFlags::ANON_OK`].
/// Displays an inline error banner when `auth_failed` is `true`.
///
/// # Arguments
/// * `parent`      - Transient parent window.
/// * `uri`         - The remote URI that requires authentication.
/// * `message`     - Human-readable prompt from the GVFS backend.
/// * `flags`       - Which credential fields are required / optional.
/// * `auth_failed` - Whether a previous attempt was rejected (shows error hint).
/// * `sender`      - Receives [`AppMsg::ConnectToServer`] with populated credentials.
pub fn show_credentials_dialog(
    parent: &impl IsA<gtk::Window>,
    uri: String,
    message: String,
    flags: NetworkAuthFlags,
    auth_failed: bool,
    sender: Sender<AppMsg>,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&crate::i18n::tr("Authentication Required"));
    dialog.set_can_close(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_width_request(360);

    // ── Error banner ──────────────────────────────────────────────────────────

    if auth_failed {
        let banner = adw::Banner::new(&crate::i18n::tr("Incorrect credentials. Please try again."));
        banner.set_revealed(true);
        banner.add_css_class("error");
        content.append(&banner);
    }

    // ── Description ───────────────────────────────────────────────────────────

    let desc = gtk::Label::new(Some(&message));
    desc.set_wrap(true);
    desc.set_halign(gtk::Align::Start);
    desc.add_css_class("dim-label");
    content.append(&desc);

    let group = adw::PreferencesGroup::new();

    // ── Anonymous toggle (optional) ───────────────────────────────────────────

    let anon_switch = if flags.contains(NetworkAuthFlags::ANON_OK) {
        let row = adw::SwitchRow::new();
        row.set_title(&crate::i18n::tr("Connect anonymously"));
        group.add(&row);
        Some(row)
    } else {
        None
    };

    // ── Username ──────────────────────────────────────────────────────────────

    let username_row = if flags.contains(NetworkAuthFlags::USERNAME) {
        let row = adw::EntryRow::new();
        row.set_title(&crate::i18n::tr("Username"));
        group.add(&row);
        Some(row)
    } else {
        None
    };

    // ── Domain (SMB) ──────────────────────────────────────────────────────────

    let domain_row = if flags.contains(NetworkAuthFlags::DOMAIN) {
        let row = adw::EntryRow::new();
        row.set_title(&crate::i18n::tr("Domain"));
        group.add(&row);
        Some(row)
    } else {
        None
    };

    // ── Password ──────────────────────────────────────────────────────────────

    let password_row = if flags.contains(NetworkAuthFlags::PASSWORD) {
        let row = adw::PasswordEntryRow::new();
        row.set_title(&crate::i18n::tr("Password"));
        group.add(&row);
        Some(row)
    } else {
        None
    };

    content.append(&group);

    // ── Buttons ───────────────────────────────────────────────────────────────

    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    button_box.set_halign(gtk::Align::End);

    let cancel_btn = gtk::Button::with_label(&crate::i18n::tr("Cancel"));
    cancel_btn.add_css_class("pill");

    let connect_btn = gtk::Button::with_label(&crate::i18n::tr("Connect"));
    connect_btn.add_css_class("pill");
    connect_btn.add_css_class("suggested-action");

    button_box.append(&cancel_btn);
    button_box.append(&connect_btn);
    content.append(&button_box);

    // ── Anonymous toggle wires field sensitivity ───────────────────────────────

    if let Some(ref anon) = anon_switch {
        let username_row_c = username_row.clone();
        let domain_row_c = domain_row.clone();
        let password_row_c = password_row.clone();

        anon.connect_active_notify(move |sw| {
            let is_anon = sw.is_active();
            if let Some(ref r) = username_row_c {
                r.set_sensitive(!is_anon);
            }
            if let Some(ref r) = domain_row_c {
                r.set_sensitive(!is_anon);
            }
            if let Some(ref r) = password_row_c {
                r.set_sensitive(!is_anon);
            }
        });
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));

    // ── Cancel ────────────────────────────────────────────────────────────────

    let dialog_c = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_c.close();
    });

    // ── Connect ───────────────────────────────────────────────────────────────

    let dialog_ok = dialog.clone();
    connect_btn.connect_clicked(move |_| {
        use crate::services::network::NetworkCredentials;

        let credentials = if anon_switch.as_ref().map(|s| s.is_active()).unwrap_or(false) {
            NetworkCredentials::anonymous()
        } else {
            NetworkCredentials {
                username: username_row
                    .as_ref()
                    .map(|r| r.text().trim().to_owned())
                    .filter(|s| !s.is_empty()),
                password: password_row
                    .as_ref()
                    .map(|r| r.text().trim().to_owned())
                    .filter(|s| !s.is_empty()),
                domain: domain_row
                    .as_ref()
                    .map(|r| r.text().trim().to_owned())
                    .filter(|s| !s.is_empty()),
                anonymous: false,
            }
        };

        let _ = sender.send(AppMsg::ConnectToServer {
            uri: uri.clone(),
            credentials: Some(credentials),
        });

        dialog_ok.close();
    });

    dialog.present(parent);
}

// ─── "Connect to Server" menu shortcut helper ─────────────────────────────────

/// Wires the "Connect to Server" button in the header/hamburger menu.
///
/// Call from `ui/init.rs` or wherever the app menu is built:
/// ```rust
/// let action_connect = gio::SimpleAction::new("connect-to-server", None),
/// action_connect.connect_activate({
///     let window = window.clone(),
///     let sender = sender.clone(),
///     move |_, _| crate::ui::network_dialogs::show_connect_to_server(&window, sender.clone())
/// }),
/// app.add_action(&action_connect),
/// app.set_accels_for_action("app.connect-to-server", &["<Primary>l"]),
/// ```
pub fn register_connect_action(
    app: &adw::Application,
    window: &adw::Window,
    sender: Sender<AppMsg>,
) {
    use adw::prelude::ApplicationExt;
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
