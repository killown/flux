use crate::i18n::tr;
use crate::model::{AppMsg, Config};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("Appearance"),
            set_icon_name: Some("applications-graphics-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("Layout"),
                set_description: Some(&tr("Adjust the visual layout of the file grid and sidebar")),
                add = &adw::ActionRow {
                    set_title: &tr("UI Scale"),
                    set_subtitle: &tr("Adjust interface scaling for high-resolution displays"),
                    add_suffix = &gtk::DropDown {
                        set_valign: gtk::Align::Center,
                        set_model: Some(&{
                            let sl = gtk::StringList::new(&[]);
                            for label in ["100%", "110%", "125%", "150%", "175%", "200%"] {
                                sl.append(label);
                            }
                            sl
                        }),
                        set_selected: {
                            const SCALES: &[f64] = &[1.0, 1.10, 1.25, 1.50, 1.75, 2.0];
                            SCALES.iter()
                                .position(|&v| (v - config.ui.ui_scale).abs() < 0.01)
                                .unwrap_or(0) as u32
                        },
                        connect_selected_notify => move |drop| {
                            const SCALES: &[f64] = &[1.0, 1.10, 1.25, 1.50, 1.75, 2.0];
                            let idx = drop.selected() as usize;
                            if let Some(&scale) = SCALES.get(idx) {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetUiScale(scale));
                                }
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Default Icon Size"),
                    set_subtitle: &tr("Base size of icons in the grid view"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(config.ui.default_icon_size as f64, 16.0, 512.0, 16.0, 0.0, 0.0),
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
                    set_title: &tr("List Icon Size"),
                    set_subtitle: &tr("Icon size used in list mode"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(config.ui.list_icon_size as f64, 16.0, 128.0, 8.0, 0.0, 0.0),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetListIconSize(spin.value() as i32));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Scale Font with Icons"),
                    set_subtitle: &tr("Dynamically adjust label font size when zooming or resizing grid icons"),
                    add_suffix = &gtk::Switch {
                        set_valign: gtk::Align::Center,
                        set_active: config.ui.scale_font_with_icons,
                        connect_state_set => move |_, state| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetScaleFontWithIcons(state));
                            }
                            glib::Propagation::Proceed
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Grid Spacing"),
                    set_subtitle: &tr("Pixel spacing between items in the grid view"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(config.ui.grid_spacing as f64, 0.0, 128.0, 2.0, 0.0, 0.0),
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
                        set_adjustment: &gtk::Adjustment::new(config.ui.max_width_chars as f64, 8.0, 128.0, 1.0, 0.0, 0.0),
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
                        set_active: config.ui.expand_labels,
                        set_valign: gtk::Align::Center,
                        connect_state_set => move |_, state| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetExpandLabels(state));
                            }
                            glib::Propagation::Proceed
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Background Tint Opacity"),
                set_description: Some(&tr("Adjust the dark gradient overlay opacity for custom background images (0.0 = transparent, 1.0 = solid)")),
                add = &adw::ActionRow {
                    set_title: &tr("Window Background Opacity"),
                    set_subtitle: &tr("Tint overlay for the main background"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.bg_alpha_window,
                            0.0,
                            1.0,
                            0.05,
                            0.1,
                            0.0,
                        ),
                        set_digits: 2,
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetBackgroundAlpha {
                                    slot: crate::model::BackgroundSlot::Window,
                                    alpha: spin.value(),
                                });
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Left Sidebar Opacity"),
                    set_subtitle: &tr("Tint overlay for the left sidebar"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.bg_alpha_sidebar_left,
                            0.0,
                            1.0,
                            0.05,
                            0.1,
                            0.0,
                        ),
                        set_digits: 2,
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetBackgroundAlpha {
                                    slot: crate::model::BackgroundSlot::SidebarLeft,
                                    alpha: spin.value(),
                                });
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Right Sidebar Opacity"),
                    set_subtitle: &tr("Tint overlay for the search / tag panel"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.bg_alpha_sidebar_right,
                            0.0,
                            1.0,
                            0.05,
                            0.1,
                            0.0,
                        ),
                        set_digits: 2,
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetBackgroundAlpha {
                                    slot: crate::model::BackgroundSlot::SidebarRight,
                                    alpha: spin.value(),
                                });
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
                        set_active: config.ui.start_maximized,
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
                        set_active: config.ui.show_csd,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShowCsd(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Window Controls on Left"),
                    set_subtitle: &tr("Move close/minimize/maximize buttons to the left"),
                    set_sensitive: config.ui.show_csd,
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.window_controls_left,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetWindowControlsLeft(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Startup Width"),
                    set_subtitle: &tr("Initial window width in pixels"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(config.ui.startup_window_width as f64, 400.0, 7680.0, 10.0, 0.0, 0.0),
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
                        set_adjustment: &gtk::Adjustment::new(config.ui.startup_window_height as f64, 300.0, 4320.0, 10.0, 0.0, 0.0),
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

                            for theme in crate::utils::helpers::list_available_themes() {
                                sl.append(&theme);
                            }
                            sl
                        }),
                        set_selected: {
                            let current = config.ui.theme.as_deref().unwrap_or("default");
                            let mut selected_idx = 0u32;

                            if current != "default" {
                                let themes = crate::utils::helpers::list_available_themes();
                                if let Some(pos) = themes.iter().position(|x| x == current) {
                                    selected_idx = (pos + 1) as u32;
                                }
                            }
                            selected_idx
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
                        set_active: config.ui.show_recents,
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
                        set_adjustment: &gtk::Adjustment::new(config.ui.recents_row as f64, 0.0, 999.0, 1.0, 0.0, 0.0),
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
                        set_active: config.ui.show_xdg_dirs,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShowXdgDirs(switch.is_active()));
                            }
                        }
                    }
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Generated Extension Icons"),
                set_description: Some(&tr("Settings for dynamically generated icons when system themes lack a dedicated icon")),
                add = &adw::ActionRow {
                    set_title: &tr("Auto-Generate Icons"),
                    set_subtitle: &tr("Synthesize SVG icons for unknown file extensions using template.svg"),
                    add_suffix = &gtk::Switch {
                        set_active: config.ui.auto_generate_mime_icons,
                        set_valign: gtk::Align::Center,
                        connect_active_notify => move |switch| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetAutoGenerateMimeIcons(switch.is_active()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Accent Color"),
                    set_subtitle: &tr("Hex color code for extension badges (e.g., '#1273b2')"),
                    set_sensitive: config.ui.auto_generate_mime_icons,
                    add_suffix = &gtk::Entry {
                        set_text: &config.ui.auto_mime_accent_color,
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("#1273b2"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            if !val.trim().is_empty() {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetAutoMimeAccentColor(val.trim().to_string()));
                                }
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Body Color"),
                    set_subtitle: &tr("Hex color code for icon body (e.g., '#e4e4e4')"),
                    set_sensitive: config.ui.auto_generate_mime_icons,
                    add_suffix = &gtk::Entry {
                        set_text: &config.ui.auto_mime_body_color,
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("#e4e4e4"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            if !val.trim().is_empty() {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetAutoMimeBodyColor(val.trim().to_string()));
                                }
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Font Color"),
                    set_subtitle: &tr("Hex color code for extension text (e.g., '#ffffff')"),
                    set_sensitive: config.ui.auto_generate_mime_icons,
                    add_suffix = &gtk::Entry {
                        set_text: &config.ui.auto_mime_font_color,
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("#ffffff"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            if !val.trim().is_empty() {
                                if let Some(s) = crate::model::SENDER.get() {
                                    let _ = s.send(AppMsg::SetAutoMimeFontColor(val.trim().to_string()));
                                }
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Base Font Size"),
                    set_subtitle: &tr("Base font scale for extension labels inside generated badges"),
                    set_sensitive: config.ui.auto_generate_mime_icons,
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.auto_mime_font_size,
                            4.0,
                            32.0,
                            0.5,
                            1.0,
                            0.0,
                        ),
                        set_digits: 1,
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetAutoMimeFontSize(spin.value()));
                            }
                        }
                    }
                },
            },
        }
    }

    page
}
