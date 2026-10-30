use crate::domain::model::{AppMsg, Config};
use crate::i18n::tr;
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;

/// Appends `groups` to a vertical box and registers it as a tab of `stack`.
fn add_tab(
    stack: &adw::ViewStack,
    name: &str,
    title: &str,
    icon: &str,
    groups: &[adw::PreferencesGroup],
) {
    let tab = gtk::Box::new(gtk::Orientation::Vertical, 24);
    for group in groups {
        tab.append(group);
    }
    stack.add_titled_with_icon(&tab, Some(name), title, icon);
}

pub fn build(config: &Config) -> adw::PreferencesPage {
    crate::hit!("build");

    // Tabs keep the page short: each one holds the groups for one topic.
    // `vhomogeneous(false)` lets the stack take the height of the visible tab only.
    let stack = adw::ViewStack::builder().vhomogeneous(false).build();
    add_tab(
        &stack,
        "layout",
        &tr("Layout"),
        "view-grid-symbolic",
        &[layout_group(config)],
    );
    add_tab(
        &stack,
        "opacity",
        &tr("Opacity"),
        "display-brightness-symbolic",
        &[opacity_group(config)],
    );
    add_tab(
        &stack,
        "window",
        &tr("Window"),
        "window-new-symbolic",
        &[window_group(config), header_buttons_group(config)],
    );
    add_tab(
        &stack,
        "theme",
        &tr("Theme"),
        "preferences-desktop-appearance-symbolic",
        &[theme_group(config)],
    );
    add_tab(
        &stack,
        "icons",
        &tr("Icons"),
        "image-x-generic-symbolic",
        &[icons_group(config)],
    );

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .halign(gtk::Align::Center)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 18);
    root.append(&switcher);
    root.append(&stack);

    let holder = adw::PreferencesGroup::new();
    holder.add(&root);

    let page = adw::PreferencesPage::builder()
        .title(tr("Appearance"))
        .icon_name("applications-graphics-symbolic")
        .build();
    page.add(&holder);
    page
}

/// Builds the "layout" settings group.
fn layout_group(config: &Config) -> adw::PreferencesGroup {
    crate::hit!("layout_group");
    relm4::view! {
        group = adw::PreferencesGroup {
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
                            if let Some(s) = crate::domain::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetUiScale(scale));
                            }
                        }
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Default Icon Size"),
                set_subtitle: &tr("Base size of icons in the grid view"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.default_icon_size as f64, 16.0, 512.0, 16.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetIconSize(spin.value() as i32));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("List Icon Size"),
                set_subtitle: &tr("Icon size used in list mode"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.list_icon_size as f64, 16.0, 128.0, 8.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetListIconSize(spin.value() as i32));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Scale Font with Icons"),
                set_subtitle: &tr("Dynamically adjust label font size when zooming or resizing grid icons"),
                set_active: config.ui.scale_font_with_icons,
                connect_active_notify => move |row| {
                    let state = row.is_active();
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetScaleFontWithIcons(state));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Grid Spacing"),
                set_subtitle: &tr("Pixel spacing between items in the grid view"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.grid_spacing as f64, 0.0, 128.0, 2.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetGridSpacing(spin.value() as i32));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Max Label Length"),
                set_subtitle: &tr("Characters before truncating filenames"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.max_width_chars as f64, 8.0, 128.0, 1.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetMaxWidthChars(spin.value() as i32));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Breadcrumbs Shown"),
                set_subtitle: &tr("Number of path segments shown in the header bar"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.breadcrumb_count as f64, 1.0, 20.0, 1.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetBreadcrumbCount(spin.value() as usize));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Breadcrumb Label Length"),
                set_subtitle: &tr("Characters before truncating breadcrumb names"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.breadcrumb_max_chars as f64, 4.0, 64.0, 1.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetBreadcrumbMaxChars(spin.value() as i32));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Expand Filenames"),
                set_subtitle: &tr("Wrap filenames across multiple lines"),
                set_active: config.ui.expand_labels,
                connect_active_notify => move |row| {
                    let state = row.is_active();
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetExpandLabels(state));
                    }
                }
            },
        }
    }

    group
}

/// Builds the "opacity" settings group.
fn opacity_group(config: &Config) -> adw::PreferencesGroup {
    relm4::view! {
        group = adw::PreferencesGroup {
            set_title: &tr("Background Tint Opacity"),
            set_description: Some(&tr("Adjust the dark gradient overlay opacity for custom background images (0.0 = transparent, 1.0 = solid)")),
            add = &adw::SpinRow {
                set_title: &tr("Window Background Opacity"),
                set_subtitle: &tr("Tint overlay for the main background"),
                set_adjustment: Some(&gtk::Adjustment::new(
                    config.ui.bg_alpha_window,
                    0.0,
                    1.0,
                    0.05,
                    0.1,
                    0.0,
                )),
                set_digits: 2,
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetBackgroundAlpha {
                            slot: crate::domain::model::BackgroundSlot::Window,
                            alpha: spin.value(),
                        });
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Left Sidebar Opacity"),
                set_subtitle: &tr("Tint overlay for the left sidebar"),
                set_adjustment: Some(&gtk::Adjustment::new(
                    config.ui.bg_alpha_sidebar_left,
                    0.0,
                    1.0,
                    0.05,
                    0.1,
                    0.0,
                )),
                set_digits: 2,
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetBackgroundAlpha {
                            slot: crate::domain::model::BackgroundSlot::SidebarLeft,
                            alpha: spin.value(),
                        });
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Right Sidebar Opacity"),
                set_subtitle: &tr("Tint overlay for the search / tag panel"),
                set_adjustment: Some(&gtk::Adjustment::new(
                    config.ui.bg_alpha_sidebar_right,
                    0.0,
                    1.0,
                    0.05,
                    0.1,
                    0.0,
                )),
                set_digits: 2,
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetBackgroundAlpha {
                            slot: crate::domain::model::BackgroundSlot::SidebarRight,
                            alpha: spin.value(),
                        });
                    }
                }
            },
        }
    }

    group
}

/// Builds the "window" settings group.
fn window_group(config: &Config) -> adw::PreferencesGroup {
    relm4::view! {
        group = adw::PreferencesGroup {
            set_title: &tr("Window"),
            set_description: Some(&tr("Window behavior and startup settings")),
            add = &adw::SwitchRow {
                set_title: &tr("Start Maximized"),
                set_subtitle: &tr("Open the application in maximized state"),
                set_active: config.ui.start_maximized,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetMaximized(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Client-Side Decorations"),
                set_subtitle: &tr("Show window controls in the header bar"),
                set_active: config.ui.show_csd,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetShowCsd(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Window Controls on Left"),
                set_subtitle: &tr("Move close/minimize/maximize buttons to the left"),
                set_sensitive: config.ui.show_csd,
                set_active: config.ui.window_controls_left,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetWindowControlsLeft(switch.is_active()));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Startup Width"),
                set_subtitle: &tr("Initial window width in pixels"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.startup_window_width as f64, 400.0, 7680.0, 10.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetWindowWidth(spin.value() as i32));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Startup Height"),
                set_subtitle: &tr("Initial window height in pixels"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.startup_window_height as f64, 300.0, 4320.0, 10.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetWindowHeight(spin.value() as i32));
                    }
                }
            },
        }
    }

    group
}

/// Builds the "header buttons" settings group.
fn header_buttons_group(config: &Config) -> adw::PreferencesGroup {
    relm4::view! {
        group = adw::PreferencesGroup {
            set_title: &tr("Header Buttons"),
            set_description: Some(&tr("Choose which buttons the header bar shows. The main menu is always visible. If the list/grid toggle is hidden, press Ctrl+Shift+,.")),
            add = &adw::SwitchRow {
                set_title: &tr("Filter Button"),
                set_subtitle: &tr("Show the filter-by-pattern button"),
                set_active: config.ui.header_show_filter,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetHeaderShowFilter(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Sort Indicator"),
                set_subtitle: &tr("Show the current sort mode and its cycle control"),
                set_active: config.ui.header_show_sort,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetHeaderShowSort(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("List/Grid Toggle"),
                set_subtitle: &tr("Show the button that switches between list and grid layout"),
                set_active: config.ui.header_show_view,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetHeaderShowView(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Preview Button"),
                set_subtitle: &tr("Show the button that toggles the preview panel"),
                set_active: config.ui.header_show_preview,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetHeaderShowPreview(switch.is_active()));
                    }
                }
            },
        }
    }

    group
}

/// Builds the "theme" settings group.
fn theme_group(config: &Config) -> adw::PreferencesGroup {
    crate::hit!("theme_group");
    relm4::view! {
        group = adw::PreferencesGroup {
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
                            if let Some(s) = crate::domain::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetTheme(if val == "default" { None } else { Some(val) }));
                            }
                        }
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Fish Shell Integration"),
                set_subtitle: &tr(
                    "Generate ~/.config/fish/conf.d/flux-theme.fish so the embedded fish shell's colors and prompt follow the Flux theme"
                ),
                set_active: config.ui.fish_theme,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetFishTheme(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Show Recents"),
                set_subtitle: &tr("Show recently visited files in the sidebar"),
                set_active: config.ui.show_recents,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetShowRecents(switch.is_active()));
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Recents Position"),
                set_subtitle: &tr("Row index of Recents in the sidebar (0 = top)"),
                set_adjustment: Some(&gtk::Adjustment::new(config.ui.recents_row as f64, 0.0, 999.0, 1.0, 0.0, 0.0)),
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetRecentsRow(spin.value() as usize));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("XDG Directories"),
                set_subtitle: &tr("Show standard user directories in sidebar"),
                set_active: config.ui.show_xdg_dirs,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetShowXdgDirs(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Sidebar Menu Needs Ctrl"),
                set_subtitle: &tr("Open sidebar context menus only with Ctrl + right-click"),
                set_active: config.ui.sidebar_menu_requires_ctrl,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetSidebarMenuRequiresCtrl(switch.is_active()));
                    }
                }
            },
            add = &adw::SwitchRow {
                set_title: &tr("Panel Menu Needs Ctrl"),
                set_subtitle: &tr("Open left sidebar, preview panel and quick list menus only with Ctrl + right-click"),
                set_active: config.ui.panel_menu_requires_ctrl,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetPanelMenuRequiresCtrl(switch.is_active()));
                    }
                }
            },
        }
    }

    group
}

/// Builds the "icons" settings group.
fn icons_group(config: &Config) -> adw::PreferencesGroup {
    crate::hit!("icons_group");
    relm4::view! {
        group = adw::PreferencesGroup {
            set_title: &tr("Generated Extension Icons"),
            set_description: Some(&tr("Settings for dynamically generated icons when system themes lack a dedicated icon")),
            add = &adw::SwitchRow {
                set_title: &tr("Auto-Generate Icons"),
                set_subtitle: &tr("Synthesize SVG icons for unknown file extensions using template.svg"),
                set_active: config.ui.auto_generate_mime_icons,
                connect_active_notify => move |switch| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetAutoGenerateMimeIcons(switch.is_active()));
                    }
                }
            },
            add = &adw::ActionRow {
                set_title: &tr("Accent Color"),
                set_subtitle: &tr("Color of the extension badge"),
                set_sensitive: config.ui.auto_generate_mime_icons,
                add_suffix = &gtk::ColorDialogButton {
                    set_valign: gtk::Align::Center,
                    set_dialog: &gtk::ColorDialog::builder().with_alpha(false).build(),
                    set_rgba: &parse_color(&config.ui.auto_mime_accent_color, "#1273b2"),
                    connect_rgba_notify => move |btn| {
                        if let Some(s) = crate::domain::model::SENDER.get() {
                            let _ = s.send(AppMsg::SetAutoMimeAccentColor(rgba_to_hex(&btn.rgba())));
                        }
                    }
                }
            },
            add = &adw::ActionRow {
                set_title: &tr("Body Color"),
                set_subtitle: &tr("Color of the icon body"),
                set_sensitive: config.ui.auto_generate_mime_icons,
                add_suffix = &gtk::ColorDialogButton {
                    set_valign: gtk::Align::Center,
                    set_dialog: &gtk::ColorDialog::builder().with_alpha(false).build(),
                    set_rgba: &parse_color(&config.ui.auto_mime_body_color, "#e4e4e4"),
                    connect_rgba_notify => move |btn| {
                        if let Some(s) = crate::domain::model::SENDER.get() {
                            let _ = s.send(AppMsg::SetAutoMimeBodyColor(rgba_to_hex(&btn.rgba())));
                        }
                    }
                }
            },
            add = &adw::ActionRow {
                set_title: &tr("Font Color"),
                set_subtitle: &tr("Color of the extension text"),
                set_sensitive: config.ui.auto_generate_mime_icons,
                add_suffix = &gtk::ColorDialogButton {
                    set_valign: gtk::Align::Center,
                    set_dialog: &gtk::ColorDialog::builder().with_alpha(false).build(),
                    set_rgba: &parse_color(&config.ui.auto_mime_font_color, "#ffffff"),
                    connect_rgba_notify => move |btn| {
                        if let Some(s) = crate::domain::model::SENDER.get() {
                            let _ = s.send(AppMsg::SetAutoMimeFontColor(rgba_to_hex(&btn.rgba())));
                        }
                    }
                }
            },
            add = &adw::SpinRow {
                set_title: &tr("Base Font Size"),
                set_subtitle: &tr("Base font scale for extension labels inside generated badges"),
                set_sensitive: config.ui.auto_generate_mime_icons,
                set_adjustment: Some(&gtk::Adjustment::new(
                    config.ui.auto_mime_font_size,
                    4.0,
                    32.0,
                    0.5,
                    1.0,
                    0.0,
                )),
                set_digits: 1,
                set_numeric: true,
                connect_value_notify => move |spin| {
                    if let Some(s) = crate::domain::model::SENDER.get() {
                        let _ = s.send(AppMsg::SetAutoMimeFontSize(spin.value()));
                    }
                }
            },
        }
    }

    group
}

/// Parses a CSS color string, falling back to `default` when it is invalid.
fn parse_color(value: &str, default: &str) -> gdk::RGBA {
    gdk::RGBA::parse(value.trim())
        .or_else(|_| gdk::RGBA::parse(default))
        .unwrap_or(gdk::RGBA::BLACK)
}

/// Formats a color as `#rrggbb`, the form the icon generator reads from the config.
fn rgba_to_hex(c: &gdk::RGBA) -> String {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        ch(c.red()),
        ch(c.green()),
        ch(c.blue())
    )
}
