use crate::i18n::tr;
use crate::model::Config;
use adw::prelude::*;
use relm4::prelude::*;

pub struct HelpWindow {
    config: Config,
}

#[derive(Debug)]
pub enum HelpMsg {}

impl HelpWindow {
    fn format_shortcut(&self, shortcut: Option<String>, default: &str) -> String {
        let raw = shortcut.unwrap_or_else(|| default.to_string());
        if raw.trim().is_empty() {
            return default.to_string();
        }
        raw.replace("<Primary>", "Ctrl + ")
            .replace("<Control>", "Ctrl + ")
            .replace("<control>", "Ctrl + ")
            .replace("<Alt>", "Alt + ")
            .replace("<Shift>", "Shift + ")
            .replace("Return", "Enter")
            .replace("BackSpace", "Backspace")
            .replace("slash", "/")
    }
}

#[relm4::component(pub)]
impl SimpleComponent for HelpWindow {
    type Init = ();
    type Input = HelpMsg;
    type Output = ();

    view! {
        adw::PreferencesWindow {
            set_title: Some(&tr("flux - Keyboard Shortcuts")),
            set_default_size: (600, 700),
            set_modal: true,
            set_search_enabled: true,
            set_resizable: true,

            // --- Navigation Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Navigation"),
                set_icon_name: Some("compass-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Navigation Shortcuts"),
                    add = &adw::ActionRow {
                        set_title: &tr("Go back in history"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.back.clone(), "Backspace"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Go forward in history"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.forward.clone(), "Alt + Right"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Open selected file or directory"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.open.clone(), "Enter"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Navigate to root directory"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.root.clone(), "/"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Open location dialog"),
                        add_suffix = &gtk::Label {
                            set_label: "Ctrl + L",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Connect to server"),
                        add_suffix = &gtk::Label {
                            set_label: "Ctrl + Shift + L",
                            add_css_class: "keycap",
                        },
                    },
                },
            },

            // --- Quick List Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Quick List"),
                set_icon_name: Some("view-list-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Quick List Shortcuts"),
                    add = &adw::ActionRow {
                        set_title: &tr("Add selection or current folder to list"),
                        add_suffix = &gtk::Label {
                            set_label: "Insert",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Pin selection or current folder to sidebar permanently"),
                        add_suffix = &gtk::Label {
                            set_label: "Ctrl + Insert",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Cycle to the next folder in the list"),
                        add_suffix = &gtk::Label {
                            set_label: "Tab",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Clear the entire list"),
                        add_suffix = &gtk::Label {
                            set_label: "Ctrl + End",
                            add_css_class: "keycap",
                        },
                    },
                },
            },

            // --- Search Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Search"),
                set_icon_name: Some("search-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Content Search"),
                    set_description: Some(&tr("Search inside file contents (not just filenames)")),

                    add = &adw::ActionRow {
                        set_title: &tr("Start content search"),
                        set_subtitle: &tr("Type colon (:) then at least 3 characters in the search bar"),
                        add_suffix = &gtk::Label {
                            set_label: ":term",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Cancel content search"),
                        set_subtitle: &tr("Press Escape while in search view"),
                        add_suffix = &gtk::Label {
                            set_label: "Esc",
                            add_css_class: "keycap",
                        },
                    },
                },
            },

            // --- System & View Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("System & View"),
                set_icon_name: Some("view-grid-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("System & View Shortcuts"),
                    add = &adw::ActionRow {
                        set_title: &tr("Rename selected item"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.rename.clone(), "F2"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Change current folder icon"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.change_icon.clone(), "F3"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Reset current folder icon to default"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.reset_icon.clone(), "Ctrl + F3"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Toggle embedded terminal"),
                        add_suffix = &gtk::Label {
                            set_label: "F4",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Refresh current directory"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.refresh.clone(), "F5"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Search files"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.search.clone(), "Ctrl + F"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Resize grid items"),
                        add_suffix = &gtk::Label {
                            set_label: "Ctrl + Scroll",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Open folder in new window"),
                        add_suffix = &gtk::Label {
                            set_label: "Ctrl + Middle Click",
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Toggle hidden files"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.toggle_hidden.clone(), "Ctrl + H"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Move selected items to trash"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.delete.clone(), "Delete"),
                            add_css_class: "keycap",
                        },
                    },
                },
            },

            // --- Application Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Application"),
                set_icon_name: Some("application-default-icon-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Application Shortcuts"),
                    add = &adw::ActionRow {
                        set_title: &tr("Open context menu editor"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.menu_editor.clone(), "F9"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Open preferences"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.settings.clone(), "F10"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Cycle through sorting modes"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.cycle_sort.clone(), "Ctrl + S"),
                            add_css_class: "keycap",
                        },
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Toggle ascending/descending sort order"),
                        add_suffix = &gtk::Label {
                            set_label: &model.format_shortcut(model.config.shortcuts.toggle_sort_order.clone(), "Ctrl + Shift + S"),
                            add_css_class: "keycap",
                        },
                    },
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = crate::utils::load_config();
        let model = HelpWindow { config };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}
