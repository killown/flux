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
        adw::Window {
            set_default_size: (550, 750),
            set_title: Some(&tr("flux - Shortcuts")),
            set_modal: true,
            set_resizable: false,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {},

                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_propagate_natural_height: true,

                    adw::PreferencesPage {
                        adw::PreferencesGroup {
                            set_title: &tr("Navigation"),
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.back.clone(), "Backspace"),
                                set_subtitle: &tr("Go back in history")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.forward.clone(), "Alt + Right"),
                                set_subtitle: &tr("Go forward in history")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.open.clone(), "Enter"),
                                set_subtitle: &tr("Open selected file or directory")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.root.clone(), "/"),
                                set_subtitle: &tr("Navigate to root directory")
                            },
                        },

                        adw::PreferencesGroup {
                            set_title: &tr("Quick List (Exclusive List)"),
                            adw::ActionRow {
                                set_title: "Insert",
                                set_subtitle: &tr("Add selection or current folder to list")
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + Insert",
                                set_subtitle: &tr("Pin selection or current folder to sidebar permanently")
                            },
                            adw::ActionRow {
                                set_title: "Tab",
                                set_subtitle: &tr("Cycle to the next folder in the list")
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + End",
                                set_subtitle: &tr("Clear the entire list")
                            },
                        },

                        adw::PreferencesGroup {
                            set_title: &tr("System & View"),
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.rename.clone(), "F2"),
                                set_subtitle: &tr("Rename selected item")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.change_icon.clone(), "F3"),
                                set_subtitle: &tr("Change current folder icon")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.reset_icon.clone(), "Ctrl + F3"),
                                set_subtitle: &tr("Reset current folder icon to default")
                            },
                            adw::ActionRow {
                                set_title: "F4",
                                set_subtitle: &tr("Toggle embedded terminal")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.refresh.clone(), "F5"),
                                set_subtitle: &tr("Refresh current directory")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.search.clone(), "Ctrl + F"),
                                set_subtitle: &tr("Search files")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.toggle_hidden.clone(), "Ctrl + H"),
                                set_subtitle: &tr("Toggle hidden files")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.delete.clone(), "Delete"),
                                set_subtitle: &tr("Move selected items to trash")
                            },
                        },

                        adw::PreferencesGroup {
                            set_title: &tr("Application"),
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.menu_editor.clone(), "F9"),
                                set_subtitle: &tr("Open context menu editor")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.settings.clone(), "F10"),
                                set_subtitle: &tr("Open preferences")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.cycle_sort.clone(), "Ctrl + S"),
                                set_subtitle: &tr("Cycle through sorting modes")
                            },
                            adw::ActionRow {
                                set_title: &model.format_shortcut(model.config.shortcuts.toggle_sort_order.clone(), "Ctrl + Shift + S"),
                                set_subtitle: &tr("Toggle ascending/descending sort order")
                            },
                        },
                    }
                }
            }
        }
    }

    fn init(
        _: Self::Init,
        _root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = HelpWindow {
            config: crate::utils::load_config(),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
