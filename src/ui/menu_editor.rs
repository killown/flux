use crate::i18n::tr;
use crate::model::MenuEntry;
use adw::prelude::*;
use gtk::glib::{self};
use relm4::prelude::*;
use std::{cell::RefCell, fs, io::Write, path::PathBuf, rc::Rc};

// ─── Parser helper (mirrors utils::split_mime_cmd) ───────────────────────────
fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>, bool)> {
    let input = input.trim();

    let remainder = input.strip_prefix('"')?;
    let (mime, rest) = remainder.split_once('"')?;

    let second_part = rest.trim().strip_prefix(',')?.trim();

    let cmd_inner = second_part.strip_prefix('"')?;
    let (cmd, after_cmd) = cmd_inner.split_once('"')?;

    let mut toast: Option<String> = None;
    let mut no_command_dialog = false;

    let mut remainder = after_cmd.trim();
    while let Some(stripped) = remainder.strip_prefix(',') {
        let stripped = stripped.trim();
        if let Some(inner) = stripped.strip_prefix('"') {
            if let Some((token, rest)) = inner.split_once('"') {
                if token == "no_command_dialog" {
                    no_command_dialog = true;
                } else {
                    toast = Some(token.to_string());
                }
                remainder = rest.trim();
                continue;
            }
        }
        break;
    }

    Some((mime.to_string(), cmd.to_string(), toast, no_command_dialog))
}

// ─── Disk I/O & Dynamic Menu Discovery ───────────────────────────────────────
fn flux_base_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux")
}

fn get_available_menus() -> Vec<String> {
    let mut menus = vec!["menu.rs".to_string()];
    let base = flux_base_dir();

    // Check root config dir
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("menu") && name.ends_with(".rs") && name != "menu.rs" {
                        menus.push(name.to_string());
                    }
                }
            }
        }
    }

    // Check menus/ subdirectory
    let sub = base.join("menus");
    if let Ok(entries) = fs::read_dir(sub) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("menu")
                        && name.ends_with(".rs")
                        && !menus.contains(&name.to_string())
                    {
                        menus.push(name.to_string());
                    }
                }
            }
        }
    }

    menus.sort();
    menus
}

fn config_path_for(menu_name: &str) -> PathBuf {
    let base = flux_base_dir();
    let sub_path = base.join("menus").join(menu_name);
    if sub_path.exists() {
        sub_path
    } else {
        let root_path = base.join(menu_name);
        if root_path.exists() || menu_name == "menu.rs" {
            root_path
        } else {
            base.join("menus").join(menu_name)
        }
    }
}

fn load_from_disk(menu_name: &str) -> Vec<MenuEntry> {
    let content = fs::read_to_string(config_path_for(menu_name)).unwrap_or_default();
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
        let Some((mime, cmd, toast, no_command_dialog)) = split_mime_cmd(right) else {
            continue;
        };
        entries.push(MenuEntry {
            label,
            submenu,
            mime_types: mime,
            command: cmd,
            toast,
            no_command_dialog,
        });
    }
    entries
}

fn write_to_disk(menu_name: &str, entries: &[MenuEntry]) -> std::io::Result<()> {
    let path = config_path_for(menu_name);
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
        target_line: usize,
    },
    Save,
    Search(String),
    SelectMenu(String),
    PromptNewMenu,
    CreateNewMenu(String),
}

// ─── Shared imperative state ──────────────────────────────────────────────────
struct Shared {
    entries: Rc<RefCell<Vec<MenuEntry>>>,
    current_menu: Rc<RefCell<String>>,
    list_box: gtk::ListBox,
    toast_overlay: adw::ToastOverlay,
    root: adw::Window,
    sender: ComponentSender<MenuEditor>,
    search_query: Rc<RefCell<String>>,
    menu_model: gtk::StringList,
    menu_dropdown: gtk::DropDown,
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
        let current_menu = Rc::new(RefCell::new("menu.rs".to_string()));
        let entries = Rc::new(RefCell::new(load_from_disk("menu.rs")));
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
        let new_menu_btn = gtk::Button::builder()
            .icon_name("document-new-symbolic")
            .tooltip_text(tr("Create menu config").as_str())
            .build();
        let save_btn = gtk::Button::builder()
            .label(tr("Save").as_str())
            .tooltip_text(tr("Write to config file  (Ctrl+S)").as_str())
            .css_classes(["suggested-action"])
            .build();

        // ── Menu Selection ComboBox / DropDown ────────────────────────────────
        let available_menus = get_available_menus();
        let default_idx = available_menus
            .iter()
            .position(|s| s == "menu.rs")
            .unwrap_or(0) as u32;

        let menu_strings: Vec<&str> = available_menus.iter().map(|s| s.as_str()).collect();
        let menu_model = gtk::StringList::new(&menu_strings);
        let menu_dropdown = gtk::DropDown::builder()
            .model(&menu_model)
            .selected(default_idx)
            .valign(gtk::Align::Center)
            .tooltip_text(tr("Select menu configuration file to edit").as_str())
            .build();

        // Pack elements into HeaderBar
        header.pack_start(&add_btn);
        header.pack_start(&new_menu_btn);

        // ── Search bar in header title position ───────────────────────────────
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(tr("Search entries…").as_str())
            .hexpand(true)
            .max_width_chars(30)
            .tooltip_text(tr("Filter entries  (Ctrl+F)").as_str())
            .build();
        header.set_title_widget(Some(&search_entry));

        // Pack menu selection dropdown and save button on the right side
        header.pack_end(&save_btn);
        header.pack_end(&menu_dropdown);
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
            current_menu: current_menu.clone(),
            list_box,
            toast_overlay,
            root: root.clone(),
            sender: sender.clone(),
            search_query,
            menu_model,
            menu_dropdown: menu_dropdown.clone(),
        }));

        rebuild_list(&shared.borrow());

        {
            let s = sender.clone();
            menu_dropdown.connect_selected_notify(move |dd| {
                if let Some(item) = dd.selected_item().and_downcast::<gtk::StringObject>() {
                    s.input(Msg::SelectMenu(item.string().to_string()));
                }
            });
        }
        {
            let s = sender.clone();
            add_btn.connect_clicked(move |_| s.input(Msg::AddEntry));
        }
        {
            let s = sender.clone();
            new_menu_btn.connect_clicked(move |_| s.input(Msg::PromptNewMenu));
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
            Msg::SelectMenu(menu_name) => {
                *shared.current_menu.borrow_mut() = menu_name.clone();
                *shared.entries.borrow_mut() = load_from_disk(&menu_name);
                rebuild_list(&shared);
            }

            Msg::PromptNewMenu => show_new_menu_dialog(&shared),

            Msg::CreateNewMenu(raw_name) => {
                let clean_name = raw_name.trim();
                let mut filename = if clean_name.starts_with("menu") {
                    clean_name.to_string()
                } else {
                    format!("menu_{}", clean_name)
                };
                if !filename.ends_with(".rs") {
                    filename.push_str(".rs");
                }

                let path = config_path_for(&filename);
                let is_new = !path.exists();
                if is_new {
                    let _ = write_to_disk(&filename, &[]);
                }

                // Update dropdown list
                let available = get_available_menus();
                let menu_strings: Vec<&str> = available.iter().map(|s| s.as_str()).collect();
                shared
                    .menu_model
                    .splice(0, shared.menu_model.n_items(), &menu_strings);

                if let Some(pos) = available.iter().position(|s| s == &filename) {
                    shared.menu_dropdown.set_selected(pos as u32);
                }

                *shared.current_menu.borrow_mut() = filename.clone();
                *shared.entries.borrow_mut() = if is_new {
                    Vec::new()
                } else {
                    load_from_disk(&filename)
                };
                rebuild_list(&shared);
            }

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

            Msg::Commit {
                entry,
                replace,
                target_line,
            } => {
                {
                    let mut entries = shared.entries.borrow_mut();
                    if let Some(old_idx) = replace {
                        if old_idx < entries.len() {
                            entries.remove(old_idx);
                        }
                    }
                    let target_idx = target_line.saturating_sub(1).min(entries.len());
                    entries.insert(target_idx, entry);
                }
                rebuild_list(&shared);
            }

            Msg::Save => {
                let menu_name = shared.current_menu.borrow().clone();
                let ok = write_to_disk(&menu_name, &shared.entries.borrow()).is_ok();
                let msg_saved = format!("{} saved", menu_name);
                let msg_failed = format!("Failed to save {}", menu_name);
                shared.toast_overlay.add_toast(if ok {
                    adw::Toast::builder()
                        .title(tr(&msg_saved).as_str())
                        .timeout(2)
                        .build()
                } else {
                    adw::Toast::builder()
                        .title(tr(&msg_failed).as_str())
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
    let raw_title = match &entry.submenu {
        Some(sub) => format!("{} › {}", sub, entry.label),
        None => entry.label.clone(),
    };

    let mut raw_subtitle = format!("{} │ {}", entry.mime_types, entry.command);
    if entry.no_command_dialog {
        raw_subtitle.push_str(" │ [no transfer dialog]");
    }

    let safe_title = glib::markup_escape_text(&raw_title);
    let safe_subtitle = glib::markup_escape_text(&raw_subtitle);

    let row = adw::ActionRow::builder()
        .title(safe_title.as_str())
        .subtitle(safe_subtitle.as_str())
        .build();

    // ── Dedicated Prefix Column Box (Line Numbers & Submenu Status) ───────────
    let prefix_grid = gtk::Grid::builder()
        .column_spacing(10)
        .valign(gtk::Align::Center)
        .margin_end(8)
        .build();

    // Column 0: Line Number (fixed width, right-aligned)
    let line_label = gtk::Label::builder()
        .label(format!("L{:02}", idx + 1))
        .css_classes(["caption", "dim-label", "numeric"])
        .halign(gtk::Align::End)
        .width_request(32)
        .build();
    prefix_grid.attach(&line_label, 0, 0, 1, 1);

    // Column 1: Submenu Column (fixed width, left-aligned)
    let sub_badge = if entry.submenu.is_some() {
        gtk::Label::builder()
            .label("sub")
            .css_classes(["caption", "accent"])
            .halign(gtk::Align::Start)
            .width_request(28)
            .build()
    } else {
        gtk::Label::builder().width_request(28).build()
    };
    prefix_grid.attach(&sub_badge, 1, 0, 1, 1);

    row.add_prefix(&prefix_grid);

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

// ─── New Menu dialog ─────────────────────────────────────────────────────────
fn show_new_menu_dialog(shared: &Shared) {
    let dialog = adw::Window::new();
    dialog.set_title(Some(tr("Create Menu").as_str()));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(&shared.root));
    dialog.set_default_size(520, -1);
    dialog.set_resizable(false);

    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    outer.append(&adw::HeaderBar::new());

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title(tr("Menu File Name").as_str())
        .description(tr("Enter a menu name (e.g. 'menu-application-all.rs' for 'application/all' MIME type)").as_str())
        .build();

    let name_entry = gtk::Entry::builder()
        .placeholder_text("custom")
        .hexpand(true)
        .margin_top(4)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let row = adw::PreferencesRow::builder().build();
    row.set_child(Some(&name_entry));
    group.add(&row);
    page.add(&group);
    outer.append(&page);

    let btn_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::End)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(20)
        .margin_end(20)
        .build();

    let cancel_btn = gtk::Button::builder().label(tr("Cancel").as_str()).build();
    let create_btn = gtk::Button::builder()
        .label(tr("Create").as_str())
        .css_classes(["suggested-action"])
        .build();

    btn_bar.append(&cancel_btn);
    btn_bar.append(&create_btn);
    outer.append(&btn_bar);

    dialog.set_content(Some(&outer));

    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

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

    {
        let d = dialog.clone();
        let sender = shared.sender.clone();
        create_btn.connect_clicked(move |_| {
            let text = name_entry.text().to_string();
            if text.trim().is_empty() {
                name_entry.add_css_class("error");
                return;
            }
            name_entry.remove_css_class("error");
            sender.input(Msg::CreateNewMenu(text));
            d.close();
        });
    }

    dialog.present();
}

// ─── Entry dialog ─────────────────────────────────────────────────────────────
fn show_dialog(shared: &Shared, replace: Option<usize>, entry: &MenuEntry) {
    let dialog = adw::Window::new();
    let dialog_title = if replace.is_some() {
        tr("Edit Entry")
    } else {
        tr("Add Entry")
    };
    dialog.set_title(Some(dialog_title.as_str()));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(&shared.root));
    dialog.set_default_size(680, -1);
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

    // ── Stacked Entry Row helper ──────────────────────────────────────────────
    let make_stacked_entry_row = |title: &str, value: &str| -> (adw::PreferencesRow, gtk::Entry) {
        let entry = gtk::Entry::builder()
            .text(value)
            .hexpand(true)
            .margin_top(4)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(8)
            .margin_bottom(4)
            .build();

        let label = gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .margin_start(12)
            .css_classes(["heading"])
            .build();

        vbox.append(&label);
        vbox.append(&entry);

        let pref_row = adw::PreferencesRow::builder().build();
        pref_row.set_child(Some(&vbox));

        (pref_row, entry)
    };

    let total_entries = shared.entries.borrow().len();
    let max_line = if replace.is_some() {
        total_entries.max(1)
    } else {
        total_entries + 1
    };

    let initial_line = replace.map(|idx| idx + 1).unwrap_or(max_line);

    // ── Line Position Spinner Row ─────────────────────────────────────────────
    let menu_name_val = shared.current_menu.borrow().clone();
    let spin_btn = gtk::SpinButton::builder()
        .adjustment(&gtk::Adjustment::new(
            initial_line as f64,
            1.0,
            max_line as f64,
            1.0,
            5.0,
            0.0,
        ))
        .numeric(true)
        .valign(gtk::Align::Center)
        .margin_end(12)
        .build();

    let line_row_title = format!("Line Position in {}", menu_name_val);
    let line_row = adw::ActionRow::builder()
        .title(tr(&line_row_title))
        .subtitle(tr("Set exact line order (1-based)").as_str())
        .activatable_widget(&spin_btn)
        .build();
    line_row.add_suffix(&spin_btn);

    let (label_row, label_entry) = make_stacked_entry_row(tr("Label").as_str(), &entry.label);
    let (sub_row, sub_entry) = make_stacked_entry_row(
        tr("Submenu (blank = top-level)").as_str(),
        entry.submenu.as_deref().unwrap_or(""),
    );
    let (mime_row, mime_entry) =
        make_stacked_entry_row(tr("MIME Types").as_str(), &entry.mime_types);
    let mime_hint = adw::ActionRow::builder()
        .title("all │ file │ directory │ trash │ image/all │ video/all │ audio/ │ text/all, application/all")
        .css_classes(["property"])
        .build();
    let (cmd_row, cmd_entry) = make_stacked_entry_row(
        tr("Command (%p = path · %d = dir · %f = filename)").as_str(),
        &entry.command,
    );
    let cmd_hint = adw::ActionRow::builder()
        .title("builtin::copy │ builtin::cut │ builtin::paste │ builtin::rename │ builtin::delete │ builtin::new_folder │ builtin::new_file │ builtin::add_to_quick_list │ builtin::set_custom_icon │ builtin::reset_custom_icon │ builtin::tagfile │ builtin::open_with")
        .css_classes(["property"])
        .build();
    let (toast_row, toast_entry) = make_stacked_entry_row(
        tr("Notification (optional)").as_str(),
        entry.toast.as_deref().unwrap_or(""),
    );

    // ── Checkbox / Switch for no_command_dialog ──────────────────────────────
    let no_transfer_switch = gtk::Switch::builder()
        .active(entry.no_command_dialog)
        .valign(gtk::Align::Center)
        .margin_end(12)
        .build();

    let no_transfer_row = adw::ActionRow::builder()
        .title(tr("Suppress Transfer Dialog").as_str())
        .subtitle(tr("Do not track execution progress or open transfer dialog").as_str())
        .activatable_widget(&no_transfer_switch)
        .build();
    no_transfer_row.add_suffix(&no_transfer_switch);

    g_id.add(&line_row);
    g_id.add(&label_row);
    g_id.add(&sub_row);
    g_act.add(&mime_row);
    g_act.add(&mime_hint);
    g_act.add(&cmd_row);
    g_act.add(&cmd_hint);
    g_act.add(&toast_row);
    g_act.add(&no_transfer_row);
    page.add(&g_id);
    page.add(&g_act);
    scroller.set_child(Some(&page));
    outer.append(&scroller);

    // ── Buttons ───────────────────────────────────────────────────────────────
    let btn_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::End)
        .spacing(8)
        .margin_top(12)
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
        .label(commit_label.as_str())
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

            let target_line = spin_btn.value_as_int() as usize;

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
                no_command_dialog: no_transfer_switch.is_active(),
            };

            sender.input(Msg::Commit {
                entry: new_entry,
                replace,
                target_line,
            });
            d.close();
        });
    }

    dialog.present();
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub fn run() {
    adw::init().expect("Failed to initialize Libadwaita");
    crate::i18n::init();
    crate::utils::helpers::load_custom_css();

    let app = adw::Application::builder()
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    RelmApp::from_app(app)
        .with_args(vec![])
        .run::<MenuEditor>(());
}
