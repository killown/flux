use crate::i18n::tr;
use crate::model::{AppMsg, Config};
use adw::prelude::*;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("Shortcuts"),
            set_icon_name: Some("org.gnome.Settings-keyboard-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("Navigation"),
                set_description: Some(&tr("Shortcuts for navigating between directories")),
                add = &adw::ActionRow {
                    set_title: &tr("Back"),
                    set_subtitle: &tr("Go to previous directory in history"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.back.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("BackSpace"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("back".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Forward"),
                    set_subtitle: &tr("Go to next directory in history"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.forward.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Alt>Right"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("forward".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Home"),
                    set_subtitle: &tr("Navigate to user's home directory"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.home.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>Home"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("home".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Root"),
                    set_subtitle: &tr("Navigate to filesystem root directory"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.root.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("slash"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("root".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Open"),
                    set_subtitle: &tr("Open selected file or directory"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.open.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("Return"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("open".to_string(), shortcut));
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Tabs"),
                set_description: Some(&tr("Shortcuts for managing browser tabs")),
                add = &adw::ActionRow {
                    set_title: &tr("New Tab"),
                    set_subtitle: &tr("Open a new tab"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.new_tab.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>t"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("new_tab".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Close Tab"),
                    set_subtitle: &tr("Close current tab"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.close_tab.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>w"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("close_tab".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Next Tab"),
                    set_subtitle: &tr("Switch to next tab"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.next_tab.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>Tab"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("next_tab".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Previous Tab"),
                    set_subtitle: &tr("Switch to previous tab"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.prev_tab.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary><Shift>Tab"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("prev_tab".to_string(), shortcut));
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("File Operations"),
                set_description: Some(&tr("Shortcuts for managing files")),
                add = &adw::ActionRow {
                    set_title: &tr("Delete"),
                    set_subtitle: &tr("Move selected items to trash"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.delete.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("Delete"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("delete".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Rename"),
                    set_subtitle: &tr("Rename selected item"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.rename.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("F2"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("rename".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Properties"),
                    set_subtitle: &tr("Inspect file properties or metadata"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.open_properties.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>i"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("open_properties".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Copy Path"),
                    set_subtitle: &tr("Copy absolute path of selected items to clipboard"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.copy_path.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary><Shift>c"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("copy_path".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Create Symbolic Link"),
                    set_subtitle: &tr("Create symlink from clipboard path"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.create_symlink.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary><Shift>v"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("create_symlink".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Create Hard Link"),
                    set_subtitle: &tr("Create hardlink from clipboard path"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.create_hardlink.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary><Alt><Shift>v"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("create_hardlink".to_string(), shortcut));
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("View & Customization"),
                set_description: Some(&tr("Shortcuts for controlling the interface")),
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Header Bar"),
                    set_subtitle: &tr("Show or hide the top navigation toolbar"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.toggle_header.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("F6"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_header".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Refresh"),
                    set_subtitle: &tr("Reload current directory"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.refresh.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("F5"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("refresh".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Search"),
                    set_subtitle: &tr("Focus search/filter bar"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.search.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>f"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("search".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Hidden"),
                    set_subtitle: &tr("Show/hide hidden files and folders"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.toggle_hidden.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>h"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_hidden".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Folders First"),
                    set_subtitle: &tr("Toggle folders first placement in current directory"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.toggle_folders_first.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("F7"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_folders_first".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Cycle Sort"),
                    set_subtitle: &tr("Cycle through sorting modes"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.cycle_sort.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>s"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("cycle_sort".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Sort Order"),
                    set_subtitle: &tr("Toggle ascending/descending order"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.toggle_sort_order.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary><Shift>s"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_sort_order".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Change Icon"),
                    set_subtitle: &tr("Open icon picker for the current folder"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.change_icon.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some(""),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("change_icon".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Reset Icon"),
                    set_subtitle: &tr("Reset folder icon to system default"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.reset_icon.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some(""),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("reset_icon".to_string(), shortcut));
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Application"),
                set_description: Some(&tr("Shortcuts for opening preferences and application tools")),
                add = &adw::ActionRow {
                    set_title: &tr("Settings"),
                    set_subtitle: &tr("Open preferences window"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.settings.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("F10"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("settings".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Menu Editor"),
                    set_subtitle: &tr("Open context menu editor"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.menu_editor.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("F9"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("menu_editor".to_string(), shortcut));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Quit"),
                    set_subtitle: &tr("Exit the application"),
                    add_suffix = &gtk::Entry {
                        set_text: config.shortcuts.quit.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("<Primary>q"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("quit".to_string(), shortcut));
                            }
                        }
                    }
                },
            },
        }
    }

    page
}
