use super::section::MetadataSection;
use super::sections::build_sections;
use crate::i18n::tr;
use crate::utils;
use adw::gio;
use adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

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
            #[watch] set_title: Some(&format!("{} - {}", tr("Properties"), model.filename)),

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

    fn init(
        path: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mime_type = utils::media::get_mime_type(&path);

        let mut file_content = Vec::new();
        if let Ok(f) = fs::File::open(&path) {
            let _ = f.take(1024 * 1024).read_to_end(&mut file_content);
        }

        let sections_data = build_sections(&path, &mime_type, &file_content);

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
        app_group.set_title(&tr("System Handler"));
        app_group.set_description(Some(&format!(
            "{} {}",
            tr("Default application for"),
            mime_type
        )));

        let mut all_apps: Vec<gio::AppInfo> = gio::AppInfo::all().into_iter().collect();
        all_apps.sort_by_key(|a| a.name().to_lowercase());

        let app_list_store = gtk::StringList::new(&[]);
        let default_app = gio::AppInfo::default_for_type(&mime_type, false);
        let mut selected_idx = gtk::INVALID_LIST_POSITION;

        for (i, app) in all_apps.iter().enumerate() {
            app_list_store.append(&app.name());
            if let Some(ref def) = default_app {
                if app.equal(def) {
                    selected_idx = i as u32;
                }
            }
        }

        let dropdown = gtk::DropDown::builder()
            .model(&app_list_store)
            .selected(selected_idx)
            .enable_search(true)
            .expression(gtk::PropertyExpression::new(
                gtk::StringObject::static_type(),
                None::<gtk::Expression>,
                "string",
            ))
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();

        let row = adw::ActionRow::builder()
            .title(tr("Open With").as_str())
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
                    self.toast_overlay
                        .add_toast(adw::Toast::new(&tr("Copied to clipboard")));
                }
            }
            PropertiesMsg::AppSelected(idx) => {
                if idx == gtk::INVALID_LIST_POSITION {
                    return;
                }
                if let Some(app) = self.app_list.get(idx as usize) {
                    let _ = app.set_as_default_for_type(&self.current_mime);

                    if let Some(id) = app.id() {
                        let _ = Command::new("xdg-mime")
                            .args(["default", id.as_ref(), &self.current_mime])
                            .status();
                    }

                    let name = app.name().to_string();
                    self.toast_overlay.add_toast(adw::Toast::new(&format!(
                        "{} {}",
                        tr("Set as default:"),
                        name
                    )));
                }
            }
        }
    }
}
