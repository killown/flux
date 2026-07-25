use crate::i18n::tr;
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
            set_title: Some(&tr("Preferences")),
            set_default_size: (800, 700),
            set_modal: true,
            set_search_enabled: true,
            add_css_class: "settings-window",

            // --- Appearance Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Appearance"),
                set_icon_name: Some("applications-graphics-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Layout"),
                    set_description: Some(&tr("Adjust the visual layout of the file grid and sidebar")),
                    add = &adw::ActionRow {
                        set_title: &tr("Default Icon Size"),
                        set_subtitle: &tr("Base size of icons in the grid view"),
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
                        set_title: &tr("Grid Spacing"),
                        set_subtitle: &tr("Pixel spacing between items in the grid view"),
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
                        set_title: &tr("Max Label Length"),
                        set_subtitle: &tr("Characters before truncating filenames"),
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
                        set_title: &tr("Expand Filenames"),
                        set_subtitle: &tr("Wrap filenames across multiple lines"),
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
                        set_title: &tr("Sidebar Width"),
                        set_subtitle: &tr("Width of the navigation pane"),
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
                },

                add = &adw::PreferencesGroup {
                    set_title: &tr("Window"),
                    set_description: Some(&tr("Window behavior and startup settings")),
                    add = &adw::ActionRow {
                        set_title: &tr("Start Maximized"),
                        set_subtitle: &tr("Open the application in maximized state"),
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
                    add = &adw::ActionRow {
                        set_title: &tr("Client-Side Decorations"),
                        set_subtitle: &tr("Show window controls in the header bar"),
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
                        set_title: &tr("Startup Width"),
                        set_subtitle: &tr("Initial window width in pixels"),
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
                        set_title: &tr("Startup Height"),
                        set_subtitle: &tr("Initial window height in pixels"),
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
                },

                add = &adw::PreferencesGroup {
                    set_title: &tr("Theme"),
                    set_description: Some(&tr("Customize the visual appearance with CSS themes")),
                    add = &adw::ActionRow {
                        set_title: &tr("Theme"),
                        set_subtitle: &tr("Select a custom CSS theme"),
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
                        set_title: &tr("Show Recents"),
                        set_subtitle: &tr("Show recently visited files in the sidebar"),
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.show_recents,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetShowRecents(switch.is_active()));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Recents Position"),
                        set_subtitle: &tr("Row index of Recents in the sidebar (0 = top)"),
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(model.config.ui.recents_row as f64, 0.0, 999.0, 1.0, 0.0, 0.0),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetRecentsRow(spin.value() as usize));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("XDG Directories"),
                        set_subtitle: &tr("Show standard user directories in sidebar"),
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
                },
            },

            // --- Thumbnails Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Thumbnails"),
                set_icon_name: Some("image-x-generic-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Preview Settings"),
                    set_description: Some(&tr("Enable or disable thumbnail generation for different file types")),
                    add = &adw::ActionRow {
                        set_title: &tr("Enable Thumbnails"),
                        set_subtitle: &tr("Show previews for supported file types"),
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.show_thumbnails,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetShowThumbnails(switch.is_active()));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Images"),
                        set_subtitle: &tr("PNG, JPG, GIF, WebP, AVIF, HEIC, BMP, TIFF, SVG"),
                        set_sensitive: model.config.ui.show_thumbnails,
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.thumbnail_types.images && model.config.ui.show_thumbnails,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetThumbnailType {
                                        type_name: "images".to_string(),
                                        enabled: switch.is_active()
                                    });
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Videos"),
                        set_subtitle: &tr("MP4, MKV, WebM, AVI, MOV, FLV, WMV"),
                        set_sensitive: model.config.ui.show_thumbnails,
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.thumbnail_types.videos && model.config.ui.show_thumbnails,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetThumbnailType {
                                        type_name: "videos".to_string(),
                                        enabled: switch.is_active()
                                    });
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Fonts"),
                        set_subtitle: &tr("TTF, OTF, WOFF, WOFF2, TTC"),
                        set_sensitive: model.config.ui.show_thumbnails,
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.thumbnail_types.fonts && model.config.ui.show_thumbnails,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetThumbnailType {
                                        type_name: "fonts".to_string(),
                                        enabled: switch.is_active()
                                    });
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("PDF Documents"),
                        set_subtitle: &tr("Portable Document Format files"),
                        set_sensitive: model.config.ui.show_thumbnails,
                        add_suffix = &gtk::Switch {
                            set_active: model.config.ui.thumbnail_types.pdfs && model.config.ui.show_thumbnails,
                            set_valign: gtk::Align::Center,
                            connect_active_notify => move |switch| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetThumbnailType {
                                        type_name: "pdfs".to_string(),
                                        enabled: switch.is_active()
                                    });
                                }
                            }
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: &tr("Preview Examples"),
                    set_description: Some(&tr("How thumbnails will appear in the file grid")),
                    add = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_halign: gtk::Align::Center,
                        set_spacing: 24,
                        set_margin_all: 12,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_halign: gtk::Align::Center,
                            set_spacing: 6,
                            gtk::Image {
                                set_icon_name: Some("image-x-generic-symbolic"),
                                set_pixel_size: 64,
                                set_opacity: if model.config.ui.thumbnail_types.images && model.config.ui.show_thumbnails { 1.0 } else { 0.3 },
                            },
                            gtk::Label {
                                set_label: &tr("Image"),
                                set_css_classes: &["caption", "dim-label"],
                            },
                            gtk::Label {
                                set_label: if model.config.ui.thumbnail_types.images && model.config.ui.show_thumbnails { "✓" } else { "✗" },
                                set_css_classes: &["caption"],
                                set_halign: gtk::Align::Center,
                                #[watch]
                                add_css_class: if model.config.ui.thumbnail_types.images && model.config.ui.show_thumbnails { "success" } else { "dim-label" },
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_halign: gtk::Align::Center,
                            set_spacing: 6,
                            gtk::Image {
                                set_icon_name: Some("video-x-generic-symbolic"),
                                set_pixel_size: 64,
                                set_opacity: if model.config.ui.thumbnail_types.videos && model.config.ui.show_thumbnails { 1.0 } else { 0.3 },
                            },
                            gtk::Label {
                                set_label: &tr("Video"),
                                set_css_classes: &["caption", "dim-label"],
                            },
                            gtk::Label {
                                set_label: if model.config.ui.thumbnail_types.videos && model.config.ui.show_thumbnails { "✓" } else { "✗" },
                                set_css_classes: &["caption"],
                                set_halign: gtk::Align::Center,
                                #[watch]
                                add_css_class: if model.config.ui.thumbnail_types.videos && model.config.ui.show_thumbnails { "success" } else { "dim-label" },
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_halign: gtk::Align::Center,
                            set_spacing: 6,
                            gtk::Image {
                                set_icon_name: Some("font-x-generic-symbolic"),
                                set_pixel_size: 64,
                                set_opacity: if model.config.ui.thumbnail_types.fonts && model.config.ui.show_thumbnails { 1.0 } else { 0.3 },
                            },
                            gtk::Label {
                                set_label: &tr("Font"),
                                set_css_classes: &["caption", "dim-label"],
                            },
                            gtk::Label {
                                set_label: if model.config.ui.thumbnail_types.fonts && model.config.ui.show_thumbnails { "✓" } else { "✗" },
                                set_css_classes: &["caption"],
                                set_halign: gtk::Align::Center,
                                #[watch]
                                add_css_class: if model.config.ui.thumbnail_types.fonts && model.config.ui.show_thumbnails { "success" } else { "dim-label" },
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_halign: gtk::Align::Center,
                            set_spacing: 6,
                            gtk::Image {
                                set_icon_name: Some("application-pdf-symbolic"),
                                set_pixel_size: 64,
                                set_opacity: if model.config.ui.thumbnail_types.pdfs && model.config.ui.show_thumbnails { 1.0 } else { 0.3 },
                            },
                            gtk::Label {
                                set_label: &tr("PDF"),
                                set_css_classes: &["caption", "dim-label"],
                            },
                            gtk::Label {
                                set_label: if model.config.ui.thumbnail_types.pdfs && model.config.ui.show_thumbnails { "✓" } else { "✗" },
                                set_css_classes: &["caption"],
                                set_halign: gtk::Align::Center,
                                #[watch]
                                add_css_class: if model.config.ui.thumbnail_types.pdfs && model.config.ui.show_thumbnails { "success" } else { "dim-label" },
                            },
                        },
                    },
                },
            },

            // --- Behavior Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Behavior"),
                set_icon_name: Some("emblem-system-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("File Operations"),
                    set_description: Some(&tr("How files and directories are opened and managed")),
                    add = &adw::ActionRow {
                        set_title: &tr("Single Click to Open"),
                        set_subtitle: &tr("Open files with a single click instead of double click"),
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
                    set_title: &tr("Sorting & Filtering"),
                    set_description: Some(&tr("How files are ordered and displayed")),
                    add = &adw::ActionRow {
                        set_title: &tr("Default Sort"),
                        set_subtitle: &tr("Primary sorting method"),
                        add_suffix = &gtk::DropDown::from_strings(&[&tr("Name"), &tr("Size"), &tr("Date"), &tr("Type")]) {
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
                        set_title: &tr("Sort Order"),
                        set_subtitle: &tr("Direction of the primary sort"),
                        add_suffix = &gtk::DropDown::from_strings(&[&tr("Ascending"), &tr("Descending")]) {
                            set_valign: gtk::Align::Center,
                            set_selected: match model.config.ui.ascending {
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
                        set_title: &tr("Folders First"),
                        set_subtitle: &tr("Display folders before files"),
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
                        set_title: &tr("Show Hidden Files"),
                        set_subtitle: &tr("Display files and folders starting with a dot"),
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
                },
            },

            // --- Terminal Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Terminal"),
                set_icon_name: Some("utilities-terminal-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Appearance"),
                    set_description: Some(&tr("Customize the embedded terminal look and feel")),
                    add = &adw::ActionRow {
                        set_title: &tr("Height (lines)"),
                        set_subtitle: &tr("Number of character lines in the terminal"),
                        add_suffix = &gtk::SpinButton {
                            set_adjustment: &gtk::Adjustment::new(
                                model.config.ui.terminal.height as f64,
                                10.0, 200.0, 5.0, 0.0, 0.0
                            ),
                            set_numeric: true,
                            set_valign: gtk::Align::Center,
                            connect_value_changed => move |spin| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetTerminalHeight(spin.value() as i32));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Font"),
                        set_subtitle: &tr("Pango font description"),
                        add_suffix = &gtk::Entry {
                            set_text: &model.config.ui.terminal.font,
                            set_valign: gtk::Align::Center,
                            connect_changed => move |entry| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetTerminalFont(entry.text().to_string()));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Foreground Color"),
                        set_subtitle: &tr("Text color (hex, e.g., '#E5E5E5')"),
                        add_suffix = &gtk::Entry {
                            set_text: &model.config.ui.terminal.fg_color,
                            set_valign: gtk::Align::Center,
                            connect_changed => move |entry| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetTerminalFgColor(entry.text().to_string()));
                                }
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: &tr("Background Color"),
                        set_subtitle: &tr("Background color (hex, e.g., '#1A1A1A')"),
                        add_suffix = &gtk::Entry {
                            set_text: &model.config.ui.terminal.bg_color,
                            set_valign: gtk::Align::Center,
                            connect_changed => move |entry| {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetTerminalBgColor(entry.text().to_string()));
                                }
                            }
                        }
                    },
                },
            },

            // --- Shortcuts Page ---
            add = &adw::PreferencesPage {
                set_title: &tr("Shortcuts"),
                set_icon_name: Some("keyboard-shortcuts-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &tr("Navigation"),
                    set_description: Some(&tr("Shortcuts for navigating between directories")),
                    add = &adw::ActionRow {
                        set_title: &tr("Back"),
                        set_subtitle: &tr("Go to previous directory in history"),
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.back.as_deref().unwrap_or(""),
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
                            set_text: model.config.shortcuts.forward.as_deref().unwrap_or(""),
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
                        set_title: &tr("Open"),
                        set_subtitle: &tr("Open selected file or directory"),
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.open.as_deref().unwrap_or(""),
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
                    set_title: &tr("File Operations"),
                    set_description: Some(&tr("Shortcuts for managing files")),
                    add = &adw::ActionRow {
                        set_title: &tr("Delete"),
                        set_subtitle: &tr("Move selected items to trash"),
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.delete.as_deref().unwrap_or(""),
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
                            set_text: model.config.shortcuts.rename.as_deref().unwrap_or(""),
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
                },

                add = &adw::PreferencesGroup {
                    set_title: &tr("View"),
                    set_description: Some(&tr("Shortcuts for controlling the interface")),
                    add = &adw::ActionRow {
                        set_title: &tr("Refresh"),
                        set_subtitle: &tr("Reload current directory"),
                        add_suffix = &gtk::Entry {
                            set_text: model.config.shortcuts.refresh.as_deref().unwrap_or(""),
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
                            set_text: model.config.shortcuts.search.as_deref().unwrap_or(""),
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
                            set_text: model.config.shortcuts.toggle_hidden.as_deref().unwrap_or(""),
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
                },
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
