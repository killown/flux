use crate::i18n::tr;
use crate::model::MenuEntry;
use adw::prelude::*;
use gtk::glib::{self};
use relm4::prelude::*;
use std::{cell::RefCell, fs, io::Write, path::PathBuf, rc::Rc};

// ─── Parser helper (mirrors utils::split_mime_cmd) ───────────────────────────
fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>)> {
    let input = input.trim();
    let (mime, rest) = input.strip_prefix('"')?.split_once('"')?;
    let second = rest.trim().strip_prefix(',')?.trim();
    let (cmd, after_cmd) = second.strip_prefix('"')?.split_once('"')?;
    let toast = after_cmd
        .trim()
        .strip_prefix(',')
        .and_then(|s| s.trim().strip_prefix('"'))
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| s.to_string());
    Some((mime.to_string(), cmd.to_string(), toast))
}

// ─── Disk I/O ────────────────────────────────────────────────────────────────
fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux/menu.rs")
}

fn load_from_disk() -> Vec<MenuEntry> {
    let content = fs::read_to_string(config_path()).unwrap_or_default();
    let mut entries = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some((left, right)) = line.split_once("=>") else {
            continue;
        };
        let full_label = left.trim().trim_matches('"');
        let (submenu, label) = match full_label.split_once(" > ") {
            Some((s, l)) => (Some(s.to_string()), l.to_string()),
            None => (None, full_label.to_string()),
        };
        let Some((mime, cmd, toast)) = split_mime_cmd(right) else {
            continue;
        };
        entries.push(MenuEntry {
            label,
            submenu,
            mime_types: mime,
            command: cmd,
            toast,
        });
    }
    entries
}

fn write_to_disk(entries: &[MenuEntry]) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&path)?;
    for entry in entries {
        writeln!(file, "{}", entry.to_config_line())?;
    }
    Ok(())
}

// ─── Component messages ──────────────────────────────────────────────────────
#[derive(Debug)]
enum Msg {
    AddEntry,
    EditEntry(usize),
    DeleteEntry(usize),
    MoveUp(usize),
    MoveDown(usize),
    Commit {
        entry: MenuEntry,
        replace: Option<usize>,
    },
    Save,
    Search(String),
}

// ─── Shared imperative state ──────────────────────────────────────────────────
struct Shared {
    entries: Rc<RefCell<Vec<MenuEntry>>>,
    list_box: gtk::ListBox,
    toast_overlay: adw::ToastOverlay,
    root: adw::Window,
    sender: ComponentSender<MenuEditor>,
    search_query: Rc<RefCell<String>>,
}

// ─── Component ───────────────────────────────────────────────────────────────
struct MenuEditor {
    shared: Rc<RefCell<Shared>>,
}

#[relm4::component]
impl SimpleComponent for MenuEditor {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        adw::Window {
            set_title: Some(tr("Flux Menu Editor").as_str()),
            set_default_size: (820, 640),
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let entries = Rc::new(RefCell::new(load_from_disk()));
        let toast_overlay = adw::ToastOverlay::new();
        let search_query = Rc::new(RefCell::new(String::new()));

        // ── Layout ───────────────────────────────────────────────────────────
        let outer_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        let header = adw::HeaderBar::new();
        let add_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(tr("Add new entry  (Ctrl+N)").as_str())
            .build();
        let save_btn = gtk::Button::builder()
            .label(tr("Save").as_str())
            .tooltip_text(tr("Write to ~/.config/flux/menu.rs  (Ctrl+S)").as_str())
            .css_classes(["suggested-action"])
            .build();

        // ── Search bar in header title position ───────────────────────────────
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(tr("Search entries…").as_str())
            .hexpand(true)
            .max_width_chars(40)
            .tooltip_text(tr("Filter entries  (Ctrl+F)").as_str())
            .build();
        header.set_title_widget(Some(&search_entry));

        header.pack_start(&add_btn);
        header.pack_end(&save_btn);
        outer_box.append(&header);

        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .build();
        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(20)
            .margin_end(20)
            .build();
        scroller.set_child(Some(&list_box));
        toast_overlay.set_child(Some(&scroller));
        outer_box.append(&toast_overlay);
        root.set_content(Some(&outer_box));

        let shared = Rc::new(RefCell::new(Shared {
            entries: entries.clone(),
            list_box,
            toast_overlay,
            root: root.clone(),
            sender: sender.clone(),
            search_query,
        }));

        rebuild_list(&shared.borrow());

        {
            let s = sender.clone();
            add_btn.connect_clicked(move |_| s.input(Msg::AddEntry));
        }
        {
            let s = sender.clone();
            save_btn.connect_clicked(move |_| s.input(Msg::Save));
        }

        // ── Live search filtering ─────────────────────────────────────────────
        {
            let s = sender.clone();
            search_entry.connect_search_changed(move |entry| {
                s.input(Msg::Search(entry.text().to_string()));
            });
        }

        // ── Global keyboard shortcuts ─────────────────────────────────────────
        let ksc = gtk::ShortcutController::new();
        ksc.set_scope(gtk::ShortcutScope::Global);
        {
            let s = sender.clone();
            ksc.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("<ctrl>n"),
                Some(gtk::CallbackAction::new(move |_, _| {
                    s.input(Msg::AddEntry);
                    glib::Propagation::Stop
                })),
            ));
        }
        {
            let s = sender.clone();
            ksc.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("<ctrl>s"),
                Some(gtk::CallbackAction::new(move |_, _| {
                    s.input(Msg::Save);
                    glib::Propagation::Stop
                })),
            ));
        }
        {
            // Ctrl+F focuses the search entry, Escape clears and blurs it
            let se = search_entry.clone();
            ksc.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("<ctrl>f"),
                Some(gtk::CallbackAction::new(move |_, _| {
                    se.grab_focus();
                    glib::Propagation::Stop
                })),
            ));
        }
        root.add_controller(ksc);

        let widgets = view_output!();
        let model = MenuEditor { shared };
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _: ComponentSender<Self>) {
        let shared = self.shared.borrow();

        match msg {
            Msg::AddEntry => show_dialog(&shared, None, &MenuEntry::default()),

            Msg::EditEntry(idx) => {
                let entry = shared
                    .entries
                    .borrow()
                    .get(idx)
                    .cloned()
                    .unwrap_or_default();
                show_dialog(&shared, Some(idx), &entry);
            }

            Msg::DeleteEntry(idx) => {
                {
                    let mut entries = shared.entries.borrow_mut();
                    if idx < entries.len() {
                        entries.remove(idx);
                    }
                }
                rebuild_list(&shared);
            }

            Msg::MoveUp(idx) => {
                let mut entries = shared.entries.borrow_mut();
                if idx > 0 {
                    entries.swap(idx - 1, idx);
                }
                drop(entries);
                rebuild_list(&shared);
            }

            Msg::MoveDown(idx) => {
                let mut entries = shared.entries.borrow_mut();
                if idx + 1 < entries.len() {
                    entries.swap(idx, idx + 1);
                }
                drop(entries);
                rebuild_list(&shared);
            }

            Msg::Commit { entry, replace } => {
                {
                    let mut entries = shared.entries.borrow_mut();
                    match replace {
                        Some(idx) if idx < entries.len() => entries[idx] = entry,
                        _ => entries.push(entry),
                    }
                }
                rebuild_list(&shared);
            }

            Msg::Save => {
                let ok = write_to_disk(&shared.entries.borrow()).is_ok();
                shared.toast_overlay.add_toast(if ok {
                    adw::Toast::builder()
                        .title(tr("menu.rs saved").as_str())
                        .timeout(2)
                        .build()
                } else {
                    adw::Toast::builder()
                        .title(tr("Failed to save menu.rs").as_str())
                        .timeout(4)
                        .build()
                });
            }

            Msg::Search(query) => {
                *shared.search_query.borrow_mut() = query;
                rebuild_list(&shared);
            }
        }
    }
}

// ─── List rebuilder ──────────────────────────────────────────────────────────
fn rebuild_list(shared: &Shared) {
    let list_box = &shared.list_box;
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    let entries = shared.entries.borrow();
    let query = shared.search_query.borrow();

    // Collect indices that survive the filter so move-up/down targets remain
    // correct relative to the canonical entries Vec, not the visual subset.
    let needle = query.trim().to_lowercase();
    let visible: Vec<(usize, &MenuEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if needle.is_empty() {
                return true;
            }
            let label_lc = e.label.to_lowercase();
            let sub_lc = e.submenu.as_deref().unwrap_or("").to_lowercase();
            let mime_lc = e.mime_types.to_lowercase();
            let cmd_lc = e.command.to_lowercase();
            label_lc.contains(&needle)
                || sub_lc.contains(&needle)
                || mime_lc.contains(&needle)
                || cmd_lc.contains(&needle)
        })
        .collect();

    if entries.is_empty() {
        list_box.append(
            &adw::ActionRow::builder()
                .title(tr("No entries yet").as_str())
                .subtitle(tr("Press + or Ctrl+N to add your first menu action").as_str())
                .build(),
        );
        return;
    }

    if visible.is_empty() {
        list_box.append(
            &adw::ActionRow::builder()
                .title(tr("No results").as_str())
                .subtitle(tr("Try a different search term").as_str())
                .build(),
        );
        return;
    }

    let total = entries.len();
    for (idx, entry) in &visible {
        list_box.append(&build_row(*idx, entry, total, &shared.sender));
    }
}

// ─── Row builder ─────────────────────────────────────────────────────────────
fn build_row(
    idx: usize,
    entry: &MenuEntry,
    total: usize,
    sender: &ComponentSender<MenuEditor>,
) -> adw::ActionRow {
    let title = match &entry.submenu {
        Some(sub) => format!("{} › {}", sub, entry.label),
        None => entry.label.clone(),
    };
    let row = adw::ActionRow::builder()
        .title(&title)
        .subtitle(format!("{} │ {}", entry.mime_types, entry.command))
        .build();

    if entry.submenu.is_some() {
        row.add_prefix(
            &gtk::Label::builder()
                .label("sub")
                .css_classes(["caption", "dim-label"])
                .valign(gtk::Align::Center)
                .build(),
        );
    }

    let btn_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .valign(gtk::Align::Center)
        .build();

    let mk = |icon: &str, tip: &str| {
        gtk::Button::builder()
            .icon_name(icon)
            .tooltip_text(tip)
            .css_classes(["flat"])
            .build()
    };
    let up = mk("go-up-symbolic", tr("Move up").as_str());
    let down = mk("go-down-symbolic", tr("Move down").as_str());
    let edit = mk("document-edit-symbolic", tr("Edit").as_str());
    let del = mk("user-trash-symbolic", tr("Delete").as_str());

    up.set_sensitive(idx > 0);
    down.set_sensitive(idx + 1 < total);
    del.add_css_class("destructive-action");

    for w in [&up, &down, &edit, &del] {
        btn_row.append(w);
    }
    row.add_suffix(&btn_row);

    macro_rules! wire {
        ($btn:expr, $msg:expr) => {{
            let s = sender.clone();
            $btn.connect_clicked(move |_| s.input($msg));
        }};
    }
    wire!(up, Msg::MoveUp(idx));
    wire!(down, Msg::MoveDown(idx));
    wire!(edit, Msg::EditEntry(idx));
    wire!(del, Msg::DeleteEntry(idx));

    row
}

// ─── Entry dialog ─────────────────────────────────────────────────────────────
fn show_dialog(shared: &Shared, replace: Option<usize>, entry: &MenuEntry) {
    let dialog = adw::Window::new();
    let dialog_title = if replace.is_some() {
        tr("Edit Entry")
    } else {
        tr("Add Entry")
    };
    dialog.set_title(Some(&dialog_title));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(&shared.root));
    dialog.set_default_size(560, -1);
    dialog.set_resizable(false);

    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    outer.append(&adw::HeaderBar::new());

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_height(true)
        .build();

    let page = adw::PreferencesPage::new();
    let g_id = adw::PreferencesGroup::builder()
        .title(tr("Identity").as_str())
        .build();
    let g_act = adw::PreferencesGroup::builder()
        .title(tr("Action").as_str())
        .build();

    // ── Fields ────────────────────────────────────────────────────────────────
    let make_entry_row = |title: &str, value: &str| -> (adw::ActionRow, gtk::Entry) {
        let entry = gtk::Entry::builder()
            .text(value)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder()
            .title(title)
            .activatable_widget(&entry)
            .build();
        row.add_suffix(&entry);
        (row, entry)
    };

    let (label_row, label_entry) = make_entry_row(tr("Label").as_str(), &entry.label);
    let (sub_row, sub_entry) = make_entry_row(
        tr("Submenu  (blank = top-level)").as_str(),
        entry.submenu.as_deref().unwrap_or(""),
    );
    let (mime_row, mime_entry) = make_entry_row(tr("MIME Types").as_str(), &entry.mime_types);
    let mime_hint = adw::ActionRow::builder()
        .title("all │ file │ directory │ trash │ image/all │ video/all │ audio/ │ text/all, application/all")
        .css_classes(["property"])
        .build();
    let (cmd_row, cmd_entry) = make_entry_row(
        tr("Command  (%p = path · %d = dir · %f = filename)").as_str(),
        &entry.command,
    );
    let cmd_hint = adw::ActionRow::builder()
        .title("builtin::copy │ builtin::cut │ builtin::paste │ builtin::open_with")
        .css_classes(["property"])
        .build();
    let (toast_row, toast_entry) = make_entry_row(
        tr("Notification  (optional)").as_str(),
        entry.toast.as_deref().unwrap_or(""),
    );

    g_id.add(&label_row);
    g_id.add(&sub_row);
    g_act.add(&mime_row);
    g_act.add(&mime_hint);
    g_act.add(&cmd_row);
    g_act.add(&cmd_hint);
    g_act.add(&toast_row);
    page.add(&g_id);
    page.add(&g_act);
    scroller.set_child(Some(&page));
    outer.append(&scroller);

    // ── Buttons ───────────────────────────────────────────────────────────────
    let btn_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::End)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(20)
        .margin_end(20)
        .build();
    let cancel_btn = gtk::Button::builder().label(tr("Cancel").as_str()).build();
    let commit_label = if replace.is_some() {
        tr("Save")
    } else {
        tr("Add")
    };
    let commit_btn = gtk::Button::builder()
        .label(&commit_label)
        .css_classes(["suggested-action"])
        .build();
    btn_bar.append(&cancel_btn);
    btn_bar.append(&commit_btn);
    outer.append(&btn_bar);

    dialog.set_content(Some(&outer));

    // ── Cancel ────────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    // ── Escape ────────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        let esc = gtk::ShortcutController::new();
        esc.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(move |_, _| {
                d.close();
                glib::Propagation::Stop
            })),
        ));
        dialog.add_controller(esc);
    }

    // ── Commit ────────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        let sender = shared.sender.clone();
        commit_btn.connect_clicked(move |_| {
            let label = label_entry.text().to_string();
            if label.trim().is_empty() {
                label_entry.add_css_class("error");
                return;
            }
            label_entry.remove_css_class("error");
            let new_entry = MenuEntry {
                label,
                submenu: {
                    let v = sub_entry.text().to_string();
                    if v.trim().is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                },
                mime_types: mime_entry.text().to_string(),
                command: cmd_entry.text().to_string(),
                toast: {
                    let v = toast_entry.text().to_string();
                    if v.trim().is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                },
            };
            sender.input(Msg::Commit {
                entry: new_entry,
                replace,
            });
            d.close();
        });
    }

    dialog.present();
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub fn run() {
    adw::init().expect("Failed to initialize Libadwaita");

    // NON_UNIQUE prevents D-Bus name registration, which avoids disrupting
    // GIO async operations (copy_async, move_async) running in the parent process
    // that share the same session bus.
    let app = adw::Application::builder()
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    RelmApp::from_app(app)
        .with_args(vec![])
        .run::<MenuEditor>(());
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── split_mime_cmd ────────────────────────────────────────────────────────

    #[test]
    fn test_split_mime_cmd_basic() {
        let input = r#""all", "builtin::copy", "Copied to clipboard""#;
        let (mime, cmd, toast) = split_mime_cmd(input).expect("must parse");
        assert_eq!(mime, "all");
        assert_eq!(cmd, "builtin::copy");
        assert_eq!(toast.as_deref(), Some("Copied to clipboard"));
    }

    #[test]
    fn test_split_mime_cmd_no_toast() {
        let input = r#""image/*", "eog {path}""#;
        let (mime, cmd, toast) = split_mime_cmd(input).expect("must parse");
        assert_eq!(mime, "image/*");
        assert_eq!(cmd, "eog {path}");
        assert!(toast.is_none());
    }

    #[test]
    fn test_split_mime_cmd_malformed_returns_none() {
        assert!(split_mime_cmd("not valid at all").is_none());
        assert!(split_mime_cmd("").is_none());
        assert!(split_mime_cmd(r#""only_mime""#).is_none());
    }

    #[test]
    fn test_split_mime_cmd_leading_trailing_whitespace() {
        let input = r#"  "text/plain", "gedit {path}"  "#;
        let (mime, cmd, _) = split_mime_cmd(input).expect("must parse despite whitespace");
        assert_eq!(mime, "text/plain");
        assert_eq!(cmd, "gedit {path}");
    }

    // ── MenuEntry round-trip ──────────────────────────────────────────────────

    #[test]
    fn test_menu_entry_to_config_line_with_submenu() {
        let entry = crate::model::MenuEntry {
            label: "Open".to_string(),
            submenu: Some("Actions".to_string()),
            mime_types: "all".to_string(),
            command: "xdg-open {path}".to_string(),
            toast: Some("Opened".to_string()),
        };

        let line = entry.to_config_line();
        assert!(line.contains("Actions > Open"));
        assert!(line.contains("xdg-open {path}"));
        assert!(line.contains("Opened"));
    }

    #[test]
    fn test_menu_entry_to_config_line_without_submenu() {
        let entry = crate::model::MenuEntry {
            label: "Copy".to_string(),
            submenu: None,
            mime_types: "all".to_string(),
            command: "builtin::copy".to_string(),
            toast: None,
        };

        let line = entry.to_config_line();
        assert!(
            !line.contains(" > "),
            "submenu separator must be absent when submenu is None"
        );
        assert!(line.contains("\"Copy\""));
        assert!(line.contains("builtin::copy"));
        assert!(!line.contains("toast"));
    }

    #[test]
    fn test_load_and_parse_round_trip() {
        let input = r#""Open With Gedit" => "text/*", "gedit {path}""#;
        let (left, right) = input.split_once("=>").expect("must have =>");
        let full_label = left.trim().trim_matches('"');
        let (submenu, label) = match full_label.split_once(" > ") {
            Some((s, l)) => (Some(s.to_string()), l.to_string()),
            None => (None, full_label.to_string()),
        };
        let (mime, cmd, toast) = split_mime_cmd(right).expect("must parse");

        assert!(submenu.is_none());
        assert_eq!(label, "Open With Gedit");
        assert_eq!(mime, "text/*");
        assert_eq!(cmd, "gedit {path}");
        assert!(toast.is_none());
    }
}
