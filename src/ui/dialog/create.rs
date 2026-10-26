use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// What a create request makes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Folder,
    File,
}

/// Result of creating one entry.
enum Created {
    Ok,
    Exists,
    /// Creation failed, the string is the OS/backend error (may be empty).
    Failed(String),
}

fn is_archive_uri(s: &str) -> bool {
    s.starts_with(crate::infra::archive::ARCHIVE_URI)
        || s.starts_with("/archive:/")
        || s.starts_with("archive://")
}

/// Splits the dialog text on commas into trimmed, non-empty names.
fn parse_names(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Creates one entry inside an archive or on the local filesystem.
/// Blocking: only call this from a worker thread, never from the GTK main thread.
fn create_blocking(
    kind: Kind,
    current_path: &Path,
    name: &str,
    archive_password: Option<&str>,
) -> Created {
    let current_str = current_path.to_string_lossy();
    if kind == Kind::Folder && is_archive_uri(&current_str) {
        let Some((archive_path, prefix)) = crate::infra::archive::parse_archive_uri(&current_str)
        else {
            return Created::Failed("invalid archive path".to_string());
        };
        let inner = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix.trim_end_matches('/'), name)
        };
        return match crate::infra::archive::create_archive_directory(
            &archive_path,
            &inner,
            archive_password,
        ) {
            Ok(()) => Created::Ok,
            Err(e) => Created::Failed(e.to_string()),
        };
    }

    let target = current_path.join(name);
    let result = match kind {
        Kind::Folder => std::fs::create_dir(&target),
        Kind::File => std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map(|_| ()),
    };
    match result {
        Ok(()) => Created::Ok,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Created::Exists,
        Err(e) => Created::Failed(e.to_string()),
    }
}

/// Creates one entry on a network location. Everything here is async so the
/// GTK main loop keeps running (mounting a share needs it to).
async fn create_network(kind: Kind, uri: &str) -> Created {
    use gtk::gio;
    let file = gio::File::for_uri(uri);

    // An unmounted share makes this fail too, which is fine: creating the
    // directory below mounts it first.
    let exists = file
        .query_info_future(
            "standard::name",
            gio::FileQueryInfoFlags::NONE,
            gtk::glib::Priority::DEFAULT,
        )
        .await
        .is_ok();
    if exists {
        return Created::Exists;
    }

    match kind {
        Kind::Folder => match crate::infra::network::create_network_directory(uri, None).await {
            Ok(()) => Created::Ok,
            Err(_) => Created::Failed(String::new()),
        },
        Kind::File => match file
            .create_future(gio::FileCreateFlags::NONE, gtk::glib::Priority::DEFAULT)
            .await
        {
            Ok(stream) => {
                let _ = stream.close_future(gtk::glib::Priority::DEFAULT).await;
                Created::Ok
            }
            Err(e) => Created::Failed(e.to_string()),
        },
    }
}

fn toast(s: &AsyncComponentSender<FluxApp>, text: String) {
    s.input(AppMsg::ShowToast(text));
}

fn failure_text(kind: Kind, is_archive: bool, detail: &str) -> String {
    let base = if is_archive {
        "Archive error".to_string()
    } else if kind == Kind::Folder {
        crate::i18n::tr("Failed to create directory")
    } else {
        crate::i18n::tr("Failed to create file")
    };
    if detail.is_empty() {
        base
    } else {
        format!("{base}: {detail}")
    }
}

/// Creates `names` one after another without ever blocking the GTK main thread:
/// filesystem and archive work runs on a worker thread, network work is async.
async fn create_entries(
    kind: Kind,
    current_path: PathBuf,
    names: Vec<String>,
    archive_password: Option<String>,
    s: AsyncComponentSender<FluxApp>,
) {
    let is_archive = kind == Kind::Folder && is_archive_uri(&current_path.to_string_lossy());
    let is_network = !is_archive && crate::infra::network::is_network_uri(&current_path);

    let mut created = 0usize;
    let mut single: Option<(Created, PathBuf)> = None;

    for name in &names {
        let (status, target) = if is_network {
            let uri = format!(
                "{}/{}",
                current_path.to_string_lossy().trim_end_matches('/'),
                name
            );
            let status = create_network(kind, &uri).await;
            (status, PathBuf::from(uri))
        } else {
            let (cp, n, pwd) = (current_path.clone(), name.clone(), archive_password.clone());
            let status =
                gtk::gio::spawn_blocking(move || create_blocking(kind, &cp, &n, pwd.as_deref()))
                    .await
                    .unwrap_or_else(|_| Created::Failed("worker thread panicked".to_string()));
            (status, current_path.join(name))
        };

        match &status {
            Created::Ok => created += 1,
            Created::Exists => {}
            Created::Failed(detail) => {
                eprintln!("flux: could not create {name:?}: {detail}");
            }
        }
        if names.len() == 1 {
            single = Some((status, target));
        }
    }

    // One name: report exactly what happened, then navigate/open as before.
    if let Some((status, target)) = single {
        match status {
            Created::Ok => match kind {
                Kind::Folder if is_archive => s.input(AppMsg::Refresh),
                Kind::Folder if is_network => s.input(AppMsg::Navigate(target)),
                Kind::Folder => s.input(AppMsg::InvalidateCacheAndNavigate(target)),
                Kind::File => {
                    FluxApp::open_file(target);
                    s.input(AppMsg::Refresh);
                }
            },
            Created::Exists => toast(&s, crate::i18n::tr("Directory or file already exists")),
            Created::Failed(detail) => toast(&s, failure_text(kind, is_archive, &detail)),
        }
        return;
    }

    // Several names: refresh once and summarise honestly.
    let total = names.len();
    let label = if kind == Kind::Folder {
        "folders"
    } else {
        "files"
    };
    s.input(AppMsg::Refresh);
    toast(
        &s,
        if created == total {
            format!("Created {created} {label}")
        } else {
            format!(
                "Created {created} of {total} {label} ({} already existed or failed)",
                total - created
            )
        },
    );
}

impl FluxApp {
    /// Displays a modal dialog prompting for single or batch folder creation.
    pub fn show_prompt_new_folder(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("show_prompt_new_folder");
        let window = gtk::Application::default().active_window();
        let current_path = self.current_path.clone();
        let archive_password = crate::infra::archive::parse_archive_uri(
            &current_path.to_string_lossy(),
        )
        .and_then(|(archive_path, _)| self.cached_archive_passwords.get(&archive_path).cloned());
        let s = sender.clone();

        let dialog = adw::AlertDialog::new(
            Some(&crate::i18n::tr("New Folder")),
            Some(&crate::i18n::tr(
                "Enter folder name(s), separated by commas for batch creation:",
            )),
        );
        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("create", &crate::i18n::tr("Create"));
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("create"));
        dialog.set_close_response("cancel");

        let entry = gtk::Entry::builder()
            .text("New Folder")
            .activates_default(true)
            .build();
        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.set_extra_child(Some(&entry));

        dialog.connect_response(None, move |_, response| {
            if response == "create" {
                let names = parse_names(entry.text().as_str());
                if !names.is_empty() {
                    relm4::spawn_local(create_entries(
                        Kind::Folder,
                        current_path.clone(),
                        names,
                        archive_password.clone(),
                        s.clone(),
                    ));
                }
            }
        });

        dialog.present(window.as_ref());
    }

    /// Displays a modal dialog prompting for single or batch file creation.
    pub fn show_prompt_new_file(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("show_prompt_new_file");
        let window = gtk::Application::default().active_window();
        let current_path = self.current_path.clone();
        let s = sender.clone();

        let dialog = adw::AlertDialog::new(
            Some(&crate::i18n::tr("New File")),
            Some(&crate::i18n::tr(
                "Enter file name(s), separated by commas for batch creation:",
            )),
        );
        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("create", &crate::i18n::tr("Create"));
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("create"));
        dialog.set_close_response("cancel");

        let entry = gtk::Entry::builder()
            .text("new_file.txt")
            .activates_default(true)
            .build();
        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.set_extra_child(Some(&entry));

        dialog.connect_response(None, move |_, response| {
            if response == "create" {
                let names = parse_names(entry.text().as_str());
                if !names.is_empty() {
                    relm4::spawn_local(create_entries(
                        Kind::File,
                        current_path.clone(),
                        names,
                        None,
                        s.clone(),
                    ));
                }
            }
        });

        dialog.present(window.as_ref());
    }
}
