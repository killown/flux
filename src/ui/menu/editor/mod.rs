//! Standalone menu editor window, `run()` is its entry point.

use crate::i18n::tr;
use crate::model::MenuEntry;
use adw::prelude::*;
use entry_dialog::show_dialog;
use gtk::glib;
use new_menu_dialog::show_new_menu_dialog;
use relm4::prelude::*;
use rows::rebuild_list;
use std::cell::RefCell;
use std::rc::Rc;
use storage::{config_path_for, get_available_menus, load_from_disk, write_to_disk};

mod builtin_dialog;
mod builtins;
mod entry_dialog;
mod new_menu_dialog;
mod rows;
mod storage;

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
