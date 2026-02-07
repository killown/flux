use adw::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Read;
use std::process::Command;
use std::os::unix::fs::MetadataExt;
use chrono::{DateTime, Local};
use adw::gio;
use sha2::{Sha256, Digest};
use crate::utils;

// ----------------------------------------------------------------------------
// Helper Functions
// ----------------------------------------------------------------------------

fn format_size(size: u64) -> String {
    if size == 0 { return "0B".to_string(); }
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let i = (size as f64).log(1024.0).floor() as usize;
    let i = i.min(units.len() - 1);
    let s = size as f64 / 1024.0f64.powi(i as i32);
    format!("{:.2} {}", s, units[i])
}

fn get_shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() { return 0.0; }
    let mut counts = [0usize; 256];
    for &b in data { counts[b as usize] += 1; }
    let len = data.len() as f64;
    let mut entropy = 0.0;
    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }
    (entropy * 10000.0).round() / 10000.0
}

fn get_git_info(path: &Path) -> Option<Vec<(String, String)>> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%h|%ai|%s", "--", &path.to_string_lossy()])
        .output()
        .ok()?;

    let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if out_str.is_empty() { return None; }

    let parts: Vec<&str> = out_str.split('|').collect();
    if parts.len() >= 3 {
        Some(vec![
            ("Hash".to_string(), parts[0].to_string()),
            ("Date".to_string(), parts[1].to_string()),
            ("Subject".to_string(), parts[2].to_string()),
        ])
    } else {
        None
    }
}

fn get_elf_info(path: &Path) -> Option<Vec<(String, String)>> {
    let mut f = fs::File::open(path).ok()?;
    let mut buffer = [0u8; 20];
    if f.read_exact(&mut buffer).is_err() { return None; }

    if &buffer[0..4] != b"\x7fELF" { return None; }

    let is_little = buffer[5] == 1;
    let endian = if is_little { "Little" } else { "Big" };

    let read_u16 = |idx: usize| -> u16 {
        if is_little {
            u16::from_le_bytes([buffer[idx], buffer[idx+1]])
        } else {
            u16::from_be_bytes([buffer[idx], buffer[idx+1]])
        }
    };

    let e_type = read_u16(16);
    let e_machine = read_u16(18);

    let type_str = match e_type {
        1 => "Relocatable", 2 => "Executable", 3 => "Shared", 4 => "Core", _ => "Unknown"
    };

    let mach_str = match e_machine {
        0x3E => "x86_64", 0x03 => "x86", 0x28 => "ARM", 0xB7 => "AArch64", _ => "Unknown"
    };

    Some(vec![
        ("Type".to_string(), type_str.to_string()),
        ("Arch".to_string(), mach_str.to_string()),
        ("Endian".to_string(), endian.to_string()),
    ])
}

fn get_text_metrics(raw: &[u8]) -> Option<Vec<(String, String)>> {
    if let Ok(content) = std::str::from_utf8(raw) {
        let lines: Vec<&str> = content.lines().collect();
        let word_count: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
        let todo_count = lines.iter().filter(|l| l.to_uppercase().contains("TODO")).count();
        let line_endings = if content.contains("\r\n") { "CRLF" } else { "LF" };

        Some(vec![
            ("Line Count".to_string(), lines.len().to_string()),
            ("Word Count".to_string(), word_count.to_string()),
            ("TODOs".to_string(), todo_count.to_string()),
            ("Line Endings".to_string(), line_endings.to_string()),
        ])
    } else {
        None
    }
}

// ----------------------------------------------------------------------------
// Factories
// ----------------------------------------------------------------------------

pub struct PropertyRow { key: String, value: String }

#[relm4::factory(pub)]
impl FactoryComponent for PropertyRow {
    type Init = (String, String);
    type Input = ();
    type Output = String;
    type ParentWidget = adw::PreferencesGroup;
    type CommandOutput = ();

    view! {
        adw::ActionRow {
            set_title: &self.key,
            set_subtitle: &self.value,
            set_activatable: true,
            add_suffix = &gtk::Image::from_icon_name("edit-copy-symbolic"),
            connect_activated[sender, val = self.value.clone()] => move |_| {
                let _ = sender.output(val.clone());
            }
        }
    }
    fn init_model(init: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        Self { key: init.0, value: init.1 }
    }
}

pub struct MetadataSection { title: String, rows: FactoryVecDeque<PropertyRow> }

#[relm4::factory(pub)]
impl FactoryComponent for MetadataSection {
    type Init = (String, Vec<(String, String)>);
    type Input = String;
    type Output = String;
    type ParentWidget = gtk::Box;
    type CommandOutput = ();

    view! {
        adw::PreferencesGroup {
            set_title: &self.title,
            #[local_ref]
            add = rows_widget -> adw::PreferencesGroup, 
        }
    }
    fn init_model(init: Self::Init, _: &DynamicIndex, sender: FactorySender<Self>) -> Self {
        let mut rows = FactoryVecDeque::builder()
            .launch(adw::PreferencesGroup::new())
            .forward(sender.input_sender(), |msg| msg);
        {
            let mut guard = rows.guard();
            for (k, v) in init.1 { guard.push_back((k, v)); }
        }
        Self { title: init.0, rows }
    }
    
    fn init_widgets(&mut self, _: &DynamicIndex, root: Self::Root, _: &gtk::Widget, _: FactorySender<Self>) -> Self::Widgets {
        let rows_widget = self.rows.widget();
        let widgets = view_output!();
        widgets
    }
    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        let _ = sender.output(msg);
    }
}

// ----------------------------------------------------------------------------
// Main Component
// ----------------------------------------------------------------------------

pub struct FileProperties {
    sections: FactoryVecDeque<MetadataSection>,
    filename: String,
    app_list: Vec<gio::AppInfo>,
    current_mime: String,
    toast_overlay: adw::ToastOverlay,
}

#[derive(Debug)]
pub enum PropertiesMsg {
    CopyToClipboard(String),
    AppSelected(u32),
}

#[relm4::component(pub)]
impl SimpleComponent for FileProperties {
    type Init = PathBuf;
    type Input = PropertiesMsg;
    type Output = ();

    view! {
        adw::Window {
            set_default_size: (540, 850),
            #[watch] set_title: Some(&format!("Properties — {}", model.filename)),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                adw::HeaderBar {
                    set_show_end_title_buttons: false,
                    set_show_start_title_buttons: false,
                },

                #[local_ref]
                overlay -> adw::ToastOverlay {
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_hscrollbar_policy: gtk::PolicyType::Never,

                        adw::Clamp {
                            set_maximum_size: 600,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 24,
                                set_margin_all: 24,

                                #[local_ref]
                                app_group -> adw::PreferencesGroup {},

                                #[local_ref]
                                sections_widget -> gtk::Box {},
                            }
                        }
                    }
                }
            }
        }
    }

    fn init(path: Self::Init, _root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let mime_type = utils::get_mime_type(&path);

        let mut file_content = Vec::new();
        if let Ok(f) = fs::File::open(&path) {
            let _ = f.take(1024 * 1024).read_to_end(&mut file_content);
        }

        let mut sections_data = Vec::new();

        sections_data.push(("File Identity".to_string(), vec![
            ("Filename".to_string(), filename.clone()),
            ("Extension".to_string(), path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or("None".to_string())),
            ("MIME Type".to_string(), mime_type.clone()),
        ]));

        if let Ok(meta) = path.metadata() {
            sections_data.push(("System & Disk".to_string(), vec![
                ("Full Path".to_string(), path.to_string_lossy().to_string()),
                ("Size".to_string(), format!("{} ({} B)", format_size(meta.len()), meta.len())),
                ("Disk Usage".to_string(), format_size(meta.blocks() * 512)),
                ("Inode".to_string(), meta.ino().to_string()),
                ("Device ID".to_string(), meta.dev().to_string()),
            ]));

            let fmt_time = |t: std::time::SystemTime| -> String {
                let dt: DateTime<Local> = t.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            };
            sections_data.push(("Temporal".to_string(), vec![
                ("Created".to_string(), meta.created().map(fmt_time).unwrap_or_default()),
                ("Modified".to_string(), meta.modified().map(fmt_time).unwrap_or_default()),
                ("Accessed".to_string(), meta.accessed().map(fmt_time).unwrap_or_default()),
            ]));

            let mut security = vec![
                ("Permissions".to_string(), format!("{:03o}", meta.mode() & 0o777)),
                ("UID/GID".to_string(), format!("{}:{}", meta.uid(), meta.gid())),
            ];
            if !file_content.is_empty() {
                security.push(("Entropy".to_string(), format!("{:.4}", get_shannon_entropy(&file_content))));
                security.push(("SHA256".to_string(), format!("{:x}", Sha256::digest(&file_content))));
            }
            sections_data.push(("Security".to_string(), security));
        }

        if mime_type.contains("executable") || mime_type.contains("sharedlib") {
            if let Some(elf) = get_elf_info(&path) {
                sections_data.push(("Executable Analysis".to_string(), elf));
            }
        }

        let is_text = mime_type.contains("text") || mime_type.contains("javascript") || mime_type.contains("json") || mime_type.contains("xml");
        let text_exts = [".py", ".rs", ".toml", ".yaml", ".sh", ".md", ".txt"];
        let has_text_ext = path.extension().map(|e| text_exts.contains(&e.to_string_lossy().as_ref())).unwrap_or(false);
        if (is_text || has_text_ext) && !file_content.is_empty() {
            if let Some(metrics) = get_text_metrics(&file_content) {
                sections_data.push(("Content Metrics".to_string(), metrics));
            }
        }

        if let Some(git) = get_git_info(&path) {
            sections_data.push(("Git History".to_string(), git));
        }

        let mut sections = FactoryVecDeque::builder()
            .launch(gtk::Box::new(gtk::Orientation::Vertical, 24))
            .forward(sender.input_sender(), PropertiesMsg::CopyToClipboard);
        
        {
            let mut guard = sections.guard();
            for (title, items) in sections_data {
                guard.push_back((title, items));
            }
        }

        let toast_overlay = adw::ToastOverlay::new();

        let app_group = adw::PreferencesGroup::new();
        app_group.set_title("System Handler");
        app_group.set_description(Some(&format!("Default application for {}", mime_type)));

        let mut all_apps: Vec<gio::AppInfo> = gio::AppInfo::all().into_iter().collect();
        all_apps.sort_by_key(|a| a.name().to_lowercase());

        let app_list_store = gtk::StringList::new(&[]);
        let default_app = gio::AppInfo::default_for_type(&mime_type, false);
        let mut selected_idx = gtk::INVALID_LIST_POSITION;

        for (i, app) in all_apps.iter().enumerate() {
            app_list_store.append(&app.name());
            if let Some(ref def) = default_app {
                if app.equal(def) { selected_idx = i as u32; }
            }
        }

        let dropdown = gtk::DropDown::builder()
            .model(&app_list_store)
            .selected(selected_idx)
            .enable_search(true)
            .expression(gtk::PropertyExpression::new(
                gtk::StringObject::static_type(),
                None::<gtk::Expression>,
                "string"
            ))
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();

        let row = adw::ActionRow::builder()
            .title("Open With")
            .activatable_widget(&dropdown)
            .build();
        row.add_suffix(&dropdown);
        app_group.add(&row);

        let sender_clone = sender.clone();
        dropdown.connect_selected_notify(move |row| {
             sender_clone.input(PropertiesMsg::AppSelected(row.selected()));
        });

        let model = FileProperties {
            sections,
            filename,
            app_list: all_apps,
            current_mime: mime_type,
            toast_overlay: toast_overlay.clone(),
        };

        let sections_widget: &gtk::Box = model.sections.widget();
        let overlay = &model.toast_overlay;
        let app_group = &app_group;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            PropertiesMsg::CopyToClipboard(text) => {
                if let Some(display) = adw::gdk::Display::default() {
                    display.clipboard().set_text(&text);
                    self.toast_overlay.add_toast(adw::Toast::new("Copied to clipboard"));
                }
            },
            PropertiesMsg::AppSelected(idx) => {
                if idx == gtk::INVALID_LIST_POSITION { return; }
                if let Some(app) = self.app_list.get(idx as usize) {
                    let _ = app.set_as_default_for_type(&self.current_mime);

                    if let Some(id) = app.id() {
                         let _ = Command::new("xdg-mime")
                            .args(["default", &id.to_string(), &self.current_mime])
                            .status();
                    }

                    let name = app.name().to_string();
                    self.toast_overlay.add_toast(adw::Toast::new(&format!("Set {} as default", name)));
                }
            }
        }
    }
}
