use crate::model::{AppMsg, Config};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

pub struct SettingsWindow {
    config: Config,
}

#[relm4::component(pub)]
impl SimpleComponent for SettingsWindow {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        adw::PreferencesWindow {
            set_title: Some("Preferences"),
            set_default_size: (650, 700),
            set_modal: true,
            set_search_enabled: true,
            add_css_class: "settings-window",

            add = &adw::PreferencesPage {
                set_title: "Appearance",
                set_icon_name: Some("applications-graphics-symbolic"),
                add_css_class: "appearance-page",

                add = &adw::PreferencesGroup {
                    set_title: "Layout",
                    add_css_class: "layout-group",

                    add = &adw::ActionRow {
                        set_title: "Default Icon Size",
                        set_subtitle: "Base size of icons in the grid view",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(model.config.ui.default_icon_size as f64, 16.0, 512.0, 16.0, 0.0, 0.0),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetIconSize(spin.value() as i32));
                                }
                            }
                        }
                    },

                    add = &adw::ActionRow {
                        set_title: "Grid Spacing",
                        set_subtitle: "Pixel spacing between items in the grid view",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(model.config.ui.grid_spacing as f64, 0.0, 128.0, 2.0, 0.0, 0.0),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetGridSpacing(spin.value() as i32));
                                }
                            }
                        }
                    },

                    add = &adw::ActionRow {
                        set_title: "Max Label Length",
                        set_subtitle: "Maximum characters to show before truncating filenames",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(model.config.ui.max_width_chars as f64, 8.0, 128.0, 1.0, 0.0, 0.0),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetMaxWidthChars(spin.value() as i32));
                                }
                            }
                        }
                    },

                    add = &adw::ActionRow {
                        set_title: "Expand Filenames",
                        set_subtitle: "Show full filenames in the grid, wrapping across multiple lines",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.expand_labels,
                            set_valign: gtk::Align::Center,
                            connect_state_set => move |_, state| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetExpandLabels(state));
                                }
                                glib::Propagation::Proceed
                            }
                        }
                    },

                    add = &adw::ActionRow {
                        set_title: "Sidebar Width",
                        set_subtitle: "Default width of the navigation pane",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(model.config.ui.sidebar_width as f64, 100.0, 800.0, 10.0, 0.0, 0.0),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetSidebarWidth(spin.value() as i32));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Startup Width",
                        set_subtitle: "Initial window width in pixels",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(model.config.ui.startup_window_width as f64, 400.0, 7680.0, 10.0, 0.0, 0.0),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetWindowWidth(spin.value() as i32));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Startup Height",
                        set_subtitle: "Initial window height in pixels",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(model.config.ui.startup_window_height as f64, 300.0, 4320.0, 10.0, 0.0, 0.0),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetWindowHeight(spin.value() as i32));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Show Client-Side Decorations (CSD)",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.show_csd,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetShowCsd(switch.is_active()));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Start Maximized",
                        set_subtitle: "Open the application in maximized state",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.start_maximized,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetMaximized(switch.is_active()));
                                }
                            }
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "Theming",
                    add_css_class: "theming-group",

                    add = &adw::ActionRow {
                        set_title: "Theme",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::DropDown {
                            set_valign: gtk::Align::Center,
                            set_model: Some(&{
                                let sl = gtk::StringList::new(&[]);
                                sl.append("default");
                                if let Ok(entries) = std::fs::read_dir(dirs::data_local_dir().unwrap_or_default().join("flux/themes")) {
                                    let mut t = Vec::new();
                                    for e in entries.flatten() {
                                        if e.path().extension().is_some_and(|ext| ext == "css") {
                                            if let Some(n) = e.path().file_stem().and_then(|n| n.to_str()) {
                                                if n != "default" { t.push(n.to_string()); }
                                            }
                                        }
                                    }
                                    t.sort();
                                    for theme in t {
                                        sl.append(&theme);
                                    }
                                }
                                sl
                            }),
                            set_selected: {
                                let current = model.config.ui.theme.as_deref().unwrap_or("default");
                                let mut idx = 0;
                                if current != "default" {
                                    if let Ok(entries) = std::fs::read_dir(dirs::data_local_dir().unwrap_or_default().join("flux/themes")) {
                                        let mut t = Vec::new();
                                        for e in entries.flatten() {
                                            if e.path().extension().is_some_and(|ext| ext == "css") {
                                                if let Some(n) = e.path().file_stem().and_then(|n| n.to_str()) {
                                                    if n != "default" { t.push(n.to_string()); }
                                                }
                                            }
                                        }
                                        t.sort();
                                        if let Some(pos) = t.iter().position(|x| x == current) {
                                            idx = (pos + 1) as u32;
                                        }
                                    }
                                }
                                idx
                            },
                            connect_selected_notify => move |drop| {
                                if let Some(item) = drop.selected_item().and_downcast::<gtk::StringObject>() {
                                    let val = item.string().to_string();
                                    if let Some(s) = crate::model::SENDER.get() {
                                        let _ = s.send(AppMsg::SetTheme(if val == "default" { None } else { Some(val) }));
                                    }
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Show XDG Directories",
                        add_css_class: "settings-row",
                        set_subtitle: "Display standard user directories in the sidebar",
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.show_xdg_dirs,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetShowXdgDirs(switch.is_active()));
                                }
                            }
                        }
                    },
                }
            },

            add = &adw::PreferencesPage {
                set_title: "Behavior",
                set_icon_name: Some("emblem-system-symbolic"),
                add_css_class: "behavior-page",

                add = &adw::PreferencesGroup {
                    set_title: "File Operations",
                    add_css_class: "file-ops-group",

                    add = &adw::ActionRow {
                        set_title: "Single Click to Open",
                        set_subtitle: "Open files and directories with a single click instead of double click",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.single_click,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetSingleClick(switch.is_active()));
                                }
                            }
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "Sorting and Filtering",
                    add_css_class: "sorting-group",

                    add = &adw::ActionRow {
                        set_title: "Default Sort",
                        set_subtitle: "Primary sorting method for files",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::DropDown::from_strings(&["Name", "Size", "Date", "Type"]) {
                            set_valign: gtk::Align::Center,
                            set_selected: match model.config.ui.default_sort {
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
                        set_title: "Sort Order",
                        set_subtitle: "Direction of the primary sort",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::DropDown::from_strings(&["Ascending", "Descending"]) {
                            set_valign: gtk::Align::Center,
                            set_selected: match model.config.ui.ascending {
                                true => 0,
                                false => 1,
                            },
                            connect_selected_notify => move |drop| {
                                let asc = match drop.selected() {
                                    0 => true,
                                    _ => false,
                                };
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetAsc(asc));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Folders First",
                        set_subtitle: "Always display folders before files when sorting",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.folders_first,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetFoldersFirst(switch.is_active()));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Show Hidden Files",
                        set_subtitle: "Display files and folders that start with a dot",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.show_hidden_by_default,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetShowHidden(switch.is_active()));
                                }
                            }
                        }
                    },
                }
            },

            add = &adw::PreferencesPage {
                set_title: "Shortcuts",
                set_icon_name: Some("keyboard-shortcuts-symbolic"),
                add_css_class: "shortcuts-page",

                add = &adw::PreferencesGroup {
                    set_title: "Navigation",
                    add_css_class: "shortcuts-group",

                    add = &adw::ActionRow {
                        set_title: "Back",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.back.as_deref().unwrap_or(""),
                            set_valign: gtk::Align::Center,
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
                        set_title: "Forward",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.forward.as_deref().unwrap_or(""),
                            set_valign: gtk::Align::Center,
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
                        set_title: "Open",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.open.as_deref().unwrap_or(""),
                            set_valign: gtk::Align::Center,
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
                    set_title: "File Operations",
                    add_css_class: "shortcuts-group",

                    add = &adw::ActionRow {
                        set_title: "Delete",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.delete.as_deref().unwrap_or(""),
                            set_valign: gtk::Align::Center,
                            connect_changed => move |entry: &gtk::Entry| {
                                let val = entry.text().to_string();
                                let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetShortcut("delete".to_string(), shortcut));
                                }
                            }
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "View and Application",
                    add_css_class: "shortcuts-group",

                    add = &adw::ActionRow {
                        set_title: "Refresh",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.refresh.as_deref().unwrap_or(""),
                            set_valign: gtk::Align::Center,
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
                        set_title: "Search",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.search.as_deref().unwrap_or(""),
                            set_valign: gtk::Align::Center,
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
                        set_title: "Toggle Hidden",
                        add_css_class: "settings-row",
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.toggle_hidden.as_deref().unwrap_or(""),
                            set_valign: gtk::Align::Center,
                            connect_changed => move |entry: &gtk::Entry| {
                                let val = entry.text().to_string();
                                let shortcut = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetShortcut("toggle_hidden".to_string(), shortcut));
                                }
                            }
                        }
                    },
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = crate::utils::load_config();
        let model = SettingsWindow { config };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
