use crate::domain::model::{
    AppMsg, ArchiveMsg, Config, FileOpsMsg, GitMsg, IconsMsg, SettingsMsg, ViewMsg,
};
use crate::i18n::tr;
use adw::prelude::*;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    crate::hit!("build");
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("Behavior"),
            set_icon_name: Some("emblem-system-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("File Operations"),
                set_description: Some(&tr("How files and directories are opened and managed")),
                add = &adw::SwitchRow {
                    set_title: &tr("Single Click to Open"),
                    set_subtitle: &tr("Open files with a single click instead of double click"),
                    set_active: config.ui.single_click,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Settings(SettingsMsg::SetSingleClick(switch.is_active())));
                        }
                    }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Disable Drag and Drop"),
                    set_subtitle: &tr("Prevent moving or copying files via mouse drag"),
                    set_active: config.ui.disable_drag_and_drop,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::FileOps(FileOpsMsg::SetDisableDragAndDrop(switch.is_active())));
                        }
                    }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Highlight Empty Folders"),
                    set_subtitle: &tr("Color the border of folders that contain no items"),
                    set_active: config.ui.show_empty_dir_emblem,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Icons(IconsMsg::SetShowEmptyDirEmblem(switch.is_active())));
                        }
                    }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Highlight Symbolic Links"),
                    set_subtitle: &tr("Color the border of symbolic links"),
                    set_active: config.ui.show_symlink_emblem,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Icons(IconsMsg::SetShowSymlinkEmblem(switch.is_active())));
                        }
                    }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Archives as Folders"),
                    set_subtitle: &tr("Draw browsable archives like folders, select one to reveal the archive icon"),
                    set_active: config.ui.archives_as_folders,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Archive(ArchiveMsg::SetArchivesAsFolders(switch.is_active())));
                        }
                    }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Auto-Show Git Diff"),
                    set_subtitle: &tr("Automatically open the diff panel when selecting a modified file"),
                    set_active: config.ui.auto_show_diff,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Git(GitMsg::SetAutoShowDiff(switch.is_active())));
                        }
                    }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Default Layout"),
                    set_subtitle: &tr("Start with grid view (off) or list view (on)"),
                    set_active: config.default_list_mode,
                    connect_active_notify => move |_switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::View(ViewMsg::ToggleListMode));
                        }
                    }
                },
                add = &adw::SpinRow {
                    set_title: &tr("Auto List Mode Threshold"),
                    set_subtitle: &tr("Folders with more items than this open in list mode. 0 disables."),
                    set_adjustment: Some(&gtk::Adjustment::new(
                        config.ui.auto_list_threshold as f64,
                        0.0,
                        100_000.0,
                        100.0,
                        1_000.0,
                        0.0,
                    )),
                    set_numeric: true,
                    connect_value_notify => move |spin| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Settings(SettingsMsg::SetAutoListThreshold(spin.value() as usize)));
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
                            crate::domain::model::SortBy::Name => 0,
                            crate::domain::model::SortBy::Size => 1,
                            crate::domain::model::SortBy::Date => 2,
                            crate::domain::model::SortBy::Type => 3,
                        },
                        connect_selected_notify => move |drop| {
                            let sort = match drop.selected() {
                                0 => crate::domain::model::SortBy::Name,
                                1 => crate::domain::model::SortBy::Size,
                                2 => crate::domain::model::SortBy::Date,
                                3 => crate::domain::model::SortBy::Type,
                                _ => crate::domain::model::SortBy::Name,
                            };
                            if let Some(s) = crate::infra::SENDER.get() {
                                let _ = s.send(AppMsg::View(ViewMsg::SetDefaultSort(sort)));
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
                            if let Some(s) = crate::infra::SENDER.get() {
                                let _ = s.send(AppMsg::View(ViewMsg::SetAsc(asc)));
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
                            if let Some(s) = crate::infra::SENDER.get() {
                                let _ = s.send(AppMsg::Settings(SettingsMsg::SetHiddenExtensions(exts)));
                            }
                        }
                  }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Folders First"),
                    set_subtitle: &tr("Display folders before files"),
                    set_active: config.ui.folders_first,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Settings(SettingsMsg::SetFoldersFirst(switch.is_active())));
                        }
                    }
                },
                add = &adw::SwitchRow {
                    set_title: &tr("Show Hidden Files"),
                    set_subtitle: &tr("Display files and folders starting with a dot"),
                    set_active: config.ui.show_hidden_by_default,
                    connect_active_notify => move |switch| {
                        if let Some(s) = crate::infra::SENDER.get() {
                            let _ = s.send(AppMsg::Settings(SettingsMsg::SetShowHidden(switch.is_active())));
                        }
                    }
                },
            },
        }
    }

    page
}
