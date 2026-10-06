use crate::i18n::tr;
use crate::model::{AppMsg, Config};
use adw::prelude::*;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("Behavior"),
            set_icon_name: Some("emblem-system-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("File Operations"),
                set_description: Some(&tr("How files and directories are opened and managed")),
                add = &adw::ActionRow {
                    set_title: &tr("Single Click to Open"),
                    set_subtitle: &tr("Open files with a single click instead of double click"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.single_click,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetSingleClick(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Disable Drag and Drop"),
                    set_subtitle: &tr("Prevent moving or copying files via mouse drag"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.disable_drag_and_drop,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetDisableDragAndDrop(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Highlight Empty Folders"),
                    set_subtitle: &tr("Color the border of folders that contain no items"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.show_empty_dir_emblem,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShowEmptyDirEmblem(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Highlight Symbolic Links"),
                    set_subtitle: &tr("Color the border of symbolic links"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.show_symlink_emblem,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShowSymlinkEmblem(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Auto-Show Git Diff"),
                    set_subtitle: &tr("Automatically open the diff panel when selecting a modified file"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.auto_show_diff,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetAutoShowDiff(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Default Layout"),
                    set_subtitle: &tr("Start with grid view (off) or list view (on)"),
                    add_suffix = &gtk::Switch {
                        set_active: config.default_list_mode,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |_switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::ToggleListMode);
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Sorting & Filtering"),
                set_description: Some(&tr("How files are ordered and displayed")),
                add = &adw::ActionRow {
                    set_title: &tr("Default Sort"),
                    set_subtitle: &tr("Primary sorting method"),
                    add_suffix = &gtk::DropDown::from_strings(&[&tr("Name"), &tr("Size"), &tr("Date"), &tr("Type")]) {
                        set_valign: gtk::Align::Center,
                        set_selected: match config.ui.default_sort {
                            crate::model::SortBy::Name => 0,
                            crate::model::SortBy::Size => 1,
                            crate::model::SortBy::Date => 2,
                            crate::model::SortBy::Type => 3,
                        },
                        connect_selected_notify => move |drop| {
                            let sort = match drop.selected() {
                                0 => crate::model::SortBy::Name,
                                1 => crate::model::SortBy::Size,
                                2 => crate::model::SortBy::Date,
                                3 => crate::model::SortBy::Type,
                                _ => crate::model::SortBy::Name,
                            };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetDefaultSort(sort));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Sort Order"),
                    set_subtitle: &tr("Direction of the primary sort"),
                    add_suffix = &gtk::DropDown::from_strings(&[&tr("Ascending"), &tr("Descending")]) {
                        set_valign: gtk::Align::Center,
                        set_selected: match config.ui.ascending {
                            true => 0,
                            false => 1,
                        },
                        connect_selected_notify => move |drop| {
                            let asc = matches!(drop.selected(), 0);
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetAsc(asc));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Hide File Extensions"),
                    set_subtitle: &tr("Comma-separated extensions to hide from labels, or * for all (e.g. desktop, AppImage, tar.*)"),
                    add_suffix = &gtk::Entry {
                        set_valign: gtk::Align::Center,
                        set_width_chars: 20,
                        set_placeholder_text: Some("desktop, AppImage"),
                        set_text: &config.ui.hidden_extensions
                            .iter()
                            .map(|e| e.trim_start_matches('.'))
                            .collect::<Vec<_>>()
                            .join(", "),
                        connect_activate => move |entry| {
                            let exts = entry
                                .text()
                                .split(',')
                                .map(|s| s.trim().trim_start_matches('.').to_lowercase())
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<String>>();
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetHiddenExtensions(exts));
                            }
                        }
                  }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Folders First"),
                    set_subtitle: &tr("Display folders before files"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.folders_first,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetFoldersFirst(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Show Hidden Files"),
                    set_subtitle: &tr("Display files and folders starting with a dot"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.show_hidden_by_default,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShowHidden(switch.is_active()));
                            }
                        }
                    }
                },
            },
        }
    }

    page
}
