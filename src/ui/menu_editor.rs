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
}

// ─── Shared imperative state ──────────────────────────────────────────────────
struct Shared {
    entries: Rc<RefCell<Vec<MenuEntry>>>,
    list_box: gtk::ListBox,
    toast_overlay: adw::ToastOverlay,
    root: adw::Window,
    sender: ComponentSender<MenuEditor>,
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
            set_title: Some("Flux Menu Editor"),
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

        // ── Layout ───────────────────────────────────────────────────────────
        let outer_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        let header = adw::HeaderBar::new();
        let add_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add new entry  (Ctrl+N)")
            .build();
        let save_btn = gtk::Button::builder()
            .label("Save")
            .tooltip_text("Write to ~/.config/flux/menu.rs  (Ctrl+S)")
            .css_classes(["suggested-action"])
            .build();
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
                        .title("menu.rs saved")
                        .timeout(2)
                        .build()
                } else {
                    adw::Toast::builder()
                        .title("Failed to save menu.rs")
                        .timeout(4)
                        .build()
                });
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
    let total = entries.len();

    if total == 0 {
        list_box.append(
            &adw::ActionRow::builder()
                .title("No entries yet")
                .subtitle("Press + or Ctrl+N to add your first menu action")
                .build(),
        );
        return;
    }
    for (idx, entry) in entries.iter().enumerate() {
        list_box.append(&build_row(idx, entry, total, &shared.sender));
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
    let up = mk("go-up-symbolic", "Move up");
    let down = mk("go-down-symbolic", "Move down");
    let edit = mk("document-edit-symbolic", "Edit");
    let del = mk("user-trash-symbolic", "Delete");

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
    dialog.set_title(Some(if replace.is_some() {
        "Edit Entry"
    } else {
        "Add Entry"
    }));
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
    let g_id = adw::PreferencesGroup::builder().title("Identity").build();
    let g_act = adw::PreferencesGroup::builder().title("Action").build();

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

    let (label_row, label_entry) = make_entry_row("Label", &entry.label);
    let (sub_row, sub_entry) = make_entry_row(
        "Submenu  (blank = top-level)",
        entry.submenu.as_deref().unwrap_or(""),
    );
    let (mime_row, mime_entry) = make_entry_row("MIME Types", &entry.mime_types);
    let mime_hint = adw::ActionRow::builder()
        .title("all │ file │ directory │ trash │ image/all │ video/all │ audio/ │ text/all, application/all")
        .css_classes(["property"])
        .build();
    let (cmd_row, cmd_entry) = make_entry_row(
        "Command  (%p = path · %d = dir · %f = filename)",
        &entry.command,
    );
    let cmd_hint = adw::ActionRow::builder()
        .title("builtin::copy │ builtin::cut │ builtin::paste │ builtin::open_with")
        .css_classes(["property"])
        .build();
    let (toast_row, toast_entry) = make_entry_row(
        "Notification  (optional)",
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
    let cancel_btn = gtk::Button::builder().label("Cancel").build();
    let commit_btn = gtk::Button::builder()
        .label(if replace.is_some() { "Save" } else { "Add" })
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
    RelmApp::new("flux.MenuEditor")
        .with_args(vec![])
        .run::<MenuEditor>(());
}
