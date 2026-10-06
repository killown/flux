use adw::prelude::*;
use relm4::Sender;

use crate::model::AppMsg;
use crate::services::network::{NetworkAuthFlags, NetworkCredentials};

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
