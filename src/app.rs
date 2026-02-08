use crate::file_properties::FileProperties;
use adw::prelude::*;
use futures::StreamExt;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::help::HelpWindow;
use crate::model::{AppMsg, FluxApp, SortBy};
use crate::ui_components::{FileItem, SidebarPlace};
use crate::utils;
use adw::gdk;
use gtk::gio;
use gtk::glib;

#[relm4::component(pub)]
impl SimpleComponent for FluxApp {
    type Init = PathBuf;
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::Window {
            set_default_size: (1100, 750),
            set_title: Some("flux"),
            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender] => move |_, keyval, _, state| {

                    // 1. Priority: System Function Keys
                    if keyval == gdk::Key::F1 {
                        sender.input(AppMsg::ShowHelp);
                        return glib::Propagation::Stop;
                    }

                    if keyval == gdk::Key::F2 {
                        sender.input(AppMsg::TriggerRenameSelection);
                        return glib::Propagation::Stop;
                    }

                    // 2. Search Logic
                    let modifiers = state & gtk::accelerator_get_default_mod_mask();
                    if !modifiers.is_empty() {
                        return glib::Propagation::Proceed;
                    }

                    match keyval {
                        // Clear filter on Escape
                        gdk::Key::Escape => {
                            sender.input(AppMsg::UpdateFilter(String::new()));
                            // Also switch back to path view if desired
                            sender.input(AppMsg::SwitchHeader("path".to_string()));
                            return glib::Propagation::Stop;
                        }
                        // Handle Backspace
                        gdk::Key::BackSpace => {
                            sender.input(AppMsg::SearchBackspace);
                            return glib::Propagation::Stop;
                        }
                        // Handle Typing
                        _ => {
                            if let Some(c) = keyval.to_unicode() {
                                if !c.is_control() {
                                    // 1. Switch header FIRST so the widget becomes visible
                                    sender.input(AppMsg::SwitchHeader("search".to_string()));

                                    // 2. Send the input.
                                    // Because the widget is now becoming visible, its 'connect_show'
                                    // handler (e.grab_focus()) will trigger automatically.
                                    sender.input(AppMsg::SearchInput(c));

                                    return glib::Propagation::Stop;
                                }
                            }
                        }
                    }

                    glib::Propagation::Proceed
                }
            },
            add_controller = gtk::EventControllerKey {
                    connect_key_pressed[sender, header_view = model.header_view.clone()] => move |ctrl, keyval, _, state| {
                        if header_view == "search" || header_view == "entry" {
                            return glib::Propagation::Proceed;
                        }

                        if let Some(root) = ctrl.widget().and_then(|w| w.root()) {
                            if let Some(focus) = root.focus() {
                                // If focus is in an Entry or any editable, don't trigger search
                                if focus.type_().is_a(gtk::Editable::static_type()) {
                                    return glib::Propagation::Proceed;
                                }
                            }
                        }

                        if state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK | gdk::ModifierType::META_MASK) {
                            return glib::Propagation::Proceed;
                        }

                        if let Some(ch) = keyval.to_unicode() {
                            if ch.is_alphabetic() && !ch.is_control() {
                                sender.input(AppMsg::UpdateFilter(ch.to_string()));
                                sender.input(AppMsg::SwitchHeader("search".to_string()));
                                return glib::Propagation::Stop;
                            }
                        }
                        glib::Propagation::Proceed
                    }
                },
                add_controller = gtk::ShortcutController {
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>h").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = h_sender.input(AppMsg::ToggleHidden);
                        glib::Propagation::Stop
                    })),
                },
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("F1").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = help_sender.input(AppMsg::ShowHelp);
                        glib::Propagation::Stop
                    })),
                },
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>s").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = s_sender.input(AppMsg::CycleSort);
                        glib::Propagation::Stop
                    })),
                },

                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>f").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = f_sender.input(AppMsg::SwitchHeader("search".to_string()));
                        glib::Propagation::Stop
                    })),
                },
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Shift>s").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = s_sender_prio.input(AppMsg::CycleFolderPriority);
                        glib::Propagation::Stop
                    })),
                },
            },
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                #[name = "sidebar_container"]
                gtk::ScrolledWindow {
                    set_width_request: model.config.ui.sidebar_width,
                    add_css_class: "sidebar",
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,
                    adw::HeaderBar {
                        set_show_start_title_buttons: false,
                        set_show_end_title_buttons: false,
                        pack_start = &gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            connect_clicked => AppMsg::GoBack,
                            #[watch] set_sensitive: !model.history.is_empty(),
                        },
                        pack_start = &gtk::Button {
                            set_icon_name: "go-next-symbolic",
                            connect_clicked => AppMsg::GoForward,
                            #[watch] set_sensitive: !model.forward_stack.is_empty(),
                        },
                        #[wrap(Some)]
                        set_title_widget = &gtk::Stack {
                            #[watch] set_visible_child_name: &model.header_view,
                            set_transition_type: gtk::StackTransitionType::Crossfade,
                            add_child = &gtk::Button {
                                add_css_class: "flat",
                                #[watch] set_label: &model.current_path.to_string_lossy(),
                                connect_clicked => AppMsg::SwitchHeader("entry".to_string()),
                            } -> { set_name: "path_old" },
                            #[name = "path_entry"]
                            add_child = &gtk::ScrolledWindow {
                                    set_hscrollbar_policy: gtk::PolicyType::External,
                                    set_vscrollbar_policy: gtk::PolicyType::Never,
                                    set_halign: gtk::Align::Center,
                                    set_min_content_width: 450,

                                    gtk::Box {
                                        add_css_class: "linked",
                                        set_spacing: 0,
                                        set_halign: gtk::Align::Center,
                                        set_valign: gtk::Align::Center,
                                        #[local_ref]
                                        breadcrumb_box -> gtk::Box {
                                            set_orientation: gtk::Orientation::Horizontal,
                                        },
                                        add_controller = gtk::GestureClick {
                                            connect_released[sender] => move |_, _, _, _| {
                                                sender.input(AppMsg::SwitchHeader("entry".to_string()));
                                            }
                                        }
                                    }
                                } -> { set_name: "path" },
                            add_child = &gtk::Entry {
                                set_hexpand: false,
                                set_halign: gtk::Align::Center,
                                set_width_request: 450,
                                #[watch] set_text: &model.current_path.to_string_lossy(),
                                add_controller = gtk::EventControllerKey {
                                    connect_key_pressed[sender] => move |_, keyval, _, _| {
                                        if keyval == gdk::Key::Escape {
                                            sender.input(AppMsg::SwitchHeader("path".to_string()));
                                            return glib::Propagation::Stop;
                                        }
                                        glib::Propagation::Proceed
                                    }
                                },
                                connect_activate[sender] => move |entry| {
                                    let path_str = entry.text().to_string();
                                    if !path_str.is_empty() {
                                        sender.input(AppMsg::Navigate(PathBuf::from(path_str)));
                                    }
                                    sender.input(AppMsg::SwitchHeader("path".to_string()));
                                },
                                connect_show => |e| {
                                    e.grab_focus();
                                    e.set_position(-1);
                                }
                            } -> { set_name: "entry" },
                            add_child = &gtk::SearchEntry {
                                set_hexpand: false,
                                set_halign: gtk::Align::Center,
                                set_width_request: 450,

                                #[track = "model.filter.is_empty()"]
                                set_text: &model.filter,

                                add_controller = gtk::EventControllerKey {
                                    connect_key_pressed[sender] => move |_, keyval, _, _| {
                                        if keyval == gdk::Key::Escape {
                                            sender.input(AppMsg::SwitchHeader("path".to_string()));
                                            return glib::Propagation::Stop;
                                        }
                                        glib::Propagation::Proceed
                                    }
                                },

                                connect_search_changed[sender] => move |entry| {
                                    sender.input(AppMsg::UpdateFilter(entry.text().to_string()));
                                },

                                connect_show => |e| {
                                    e.grab_focus();
                                    e.set_position(-1);
                                },

                                connect_stop_search => AppMsg::SwitchHeader("path".to_string()),
                                add_controller = gtk::GestureClick {
                                    connect_pressed[sender] => move |_, _, _, _| {
                                        sender.input(AppMsg::SwitchHeader("entry".to_string()));
                                    }
                                },
                            } -> { set_name: "search" },
                        },
                        pack_end = &gtk::Button {
                            #[watch]
                            set_visible: model.current_path.to_string_lossy() == "trash:///",
                            connect_clicked => AppMsg::EmptyTrash,
                            set_tooltip_text: Some("Empty Trash"),
                            add_css_class: "destructive-action",

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,

                                gtk::Image {
                                    set_icon_name: Some("user-trash-full-symbolic"),
                                },

                                gtk::Label {
                                    set_label: "Empty Trash",
                                }
                            }
                        },
                        pack_end = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_end: 12,
                            set_valign: gtk::Align::Center,
                            add_css_class: "sort-container",

                            gtk::Image {
                                set_icon_name: Some("view-sort-ascending-symbolic"),
                                set_pixel_size: 16,
                                set_opacity: 0.6,
                            },

                            gtk::Label {
                                add_css_class: "sort-status-label",
                                set_opacity: 0.8,
                                #[watch]
                                set_label: model.sort_status(),
                            }
                        }
                    },
                    #[name = "grid_scroller"]
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        add_controller = gtk::EventControllerScroll {
                            set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                            connect_scroll[sender] => move |ctrl, _, dy| {
                                let modifiers = ctrl.current_event_state();
                                if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                                    sender.input(AppMsg::Zoom(dy));
                                    return glib::Propagation::Stop;
                                }
                                glib::Propagation::Proceed
                            }
                        },
                        add_controller = gtk::GestureClick {
                            set_button: 3,
                            connect_pressed[sender] => move |gesture, _, x, y| {
                                if let Some(widget) = gesture.widget() {
                                    let mut picked_path = None;
                                    if let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) {
                                        let mut current: Option<gtk::Widget> = Some(picked);
                                        while let Some(w) = current {
                                            let name = w.widget_name().to_string();
                                            if name.starts_with("/") || name.starts_with("trash://") {
                                                picked_path = Some(PathBuf::from(name));
                                                break;
                                            }
                                            current = w.parent();
                                        }
                                    }
                                 sender.input(AppMsg::PrepareContextMenu(x, y, picked_path));
                                }
                            }
                        },
                    }
                }
            }
        }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        self.handle_update(message, sender);
    }

    fn init(
        start_path: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let _ = crate::model::SENDER.set(sender.input_sender().clone());
        relm4::set_global_css(include_str!("style.css"));

        let h_sender = sender.clone();
        let s_sender = sender.clone();
        let s_sender_prio = sender.clone();
        let f_sender = sender.clone();

        let rename_sender = sender.clone();
        let help_sender = sender.clone();

        let config = utils::load_config();

        let menu_actions_list = utils::load_menu_config();

        let context_menu_popover = gtk::PopoverMenu::builder().has_arrow(false).build();

        let action_group = gio::SimpleActionGroup::new();
        let app_sender = sender.clone();

        let prio_action = gio::SimpleAction::new("cycle-priority", None);
        let prio_sender = sender.clone();
        prio_action.connect_activate(move |_, _| {
            prio_sender.input(AppMsg::CycleFolderPriority);
        });
        action_group.add_action(&prio_action);

        for action_def in &menu_actions_list {
            let cmd_clone = action_def.command.clone();
            let sender_clone = app_sender.clone();
            let action = gio::SimpleAction::new(&action_def.action_name, None);
            action.connect_activate(move |_, _| {
                sender_clone.input(AppMsg::ExecuteCommand(cmd_clone.clone()));
            });
            action_group.add_action(&action);
        }

        root.insert_action_group("win", Some(&action_group));

        let files = TypedGridView::<FileItem, gtk::MultiSelection>::new();
        files.view.set_enable_rubberband(true);

        let grid_view = &files.view;
        let sender_clone = sender.clone();
        grid_view.connect_activate(move |_, pos| sender_clone.input(AppMsg::Open(pos)));

        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder()
            .launch(listbox)
            .forward(sender.input_sender(), |path| AppMsg::Navigate(path));

        let volume_monitor = gio::VolumeMonitor::get();
        let s_added = sender.clone();
        volume_monitor.connect_mount_added(move |_, _| s_added.input(AppMsg::RefreshSidebar));

        let s = sender.clone();
        volume_monitor.connect_mount_removed(move |_, _| {
            s.input(AppMsg::RefreshSidebar);
        });

        let breadcrumb_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let breadcrumbs = FactoryVecDeque::builder()
            .launch(breadcrumb_box.clone())
            .forward(sender.input_sender(), |path| AppMsg::Navigate(path));

        let mut model = FluxApp {
            files,
            sidebar,
            breadcrumbs,
            current_path: start_path.clone(),
            history: Vec::new(),
            forward_stack: Vec::new(),
            load_id: Arc::new(AtomicU64::new(0)),
            current_icon_size: config.ui.default_icon_size,
            context_menu_popover,
            menu_actions: menu_actions_list,
            active_item_path: None,
            directory_monitor: None,
            action_group,
            sort_by: config.ui.default_sort.clone(),
            show_hidden: config.ui.show_hidden_by_default,
            config: config.clone(),
            _volume_monitor: volume_monitor,
            filter: String::new(),
            header_view: "path".to_string(),
        };

        model.update_breadcrumbs();

        for place in &config.sidebar {
            model.sidebar.guard().push_back(SidebarPlace {
                name: place.name.clone(),
                icon: place.icon.clone(),
                path: utils::expand_path(&place.path),
            });
        }

        model.refresh_sidebar();
        model.load_path(start_path, &sender);

        let widgets = view_output!();

        widgets.grid_scroller.set_child(Some(&model.files.view));
        widgets
            .sidebar_container
            .set_child(Some(model.sidebar.widget()));
        model
            .context_menu_popover
            .set_parent(&widgets.grid_scroller);

        ComponentParts { model, widgets }
    }
}

impl FluxApp {
    pub fn refresh_sidebar(&mut self) {
        let mut guard = self.sidebar.guard();
        guard.clear();

        let get_xdg_name = |p: &PathBuf| {
            gio::File::for_path(p)
                .query_info(
                    gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .map(|info| info.display_name().to_string())
                .unwrap_or_else(|_| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                })
        };

        // 1. Core XDG Directories
        if let Some(p) = dirs::home_dir() {
            guard.push_back(SidebarPlace {
                name: get_xdg_name(&p),
                icon: "user-home-symbolic".to_string(),
                path: p,
            });
        }
        if let Some(p) = dirs::desktop_dir() {
            guard.push_back(SidebarPlace {
                name: get_xdg_name(&p),
                icon: "user-desktop-symbolic".to_string(),
                path: p,
            });
        }
        if let Some(p) = dirs::download_dir() {
            guard.push_back(SidebarPlace {
                name: get_xdg_name(&p),
                icon: "folder-download-symbolic".to_string(),
                path: p,
            });
        }
        if let Some(p) = dirs::document_dir() {
            guard.push_back(SidebarPlace {
                name: get_xdg_name(&p),
                icon: "folder-documents-symbolic".to_string(),
                path: p,
            });
        }
        if let Some(p) = dirs::picture_dir() {
            guard.push_back(SidebarPlace {
                name: get_xdg_name(&p),
                icon: "folder-pictures-symbolic".to_string(),
                path: p,
            });
        }
        if let Some(p) = dirs::video_dir() {
            guard.push_back(SidebarPlace {
                name: get_xdg_name(&p),
                icon: "folder-videos-symbolic".to_string(),
                path: p,
            });
        }

        // 2. Trash (Placed below XDG directories)
        guard.push_back(SidebarPlace {
            name: "Trash".to_string(),
            icon: "user-trash-symbolic".to_string(),
            path: PathBuf::from("trash:///"),
        });

        // 3. Custom Sidebar logic
        for custom in &self.config.sidebar {
            let path = if custom.path.starts_with('~') {
                dirs::home_dir()
                    .map(|h| PathBuf::from(custom.path.replace('~', &h.to_string_lossy())))
                    .unwrap_or_else(|| PathBuf::from(&custom.path))
            } else {
                PathBuf::from(&custom.path)
            };
            guard.push_back(SidebarPlace {
                name: custom.name.clone(),
                icon: custom.icon.clone(),
                path,
            });
        }

        // 4. Mounts with Rename support
        for (mut name, path) in utils::get_system_mounts() {
            if let Some(new_name) = self.config.ui.device_renames.get(&name) {
                name = new_name.clone();
            }

            let icon = if name.to_lowercase().contains("drive")
                || name.to_lowercase().contains("cloud")
                || path.to_string_lossy().contains("Gdrive")
            {
                "folder-remote-symbolic".to_string()
            } else {
                "drive-harddisk-symbolic".to_string()
            };

            guard.push_back(SidebarPlace { name, icon, path });
        }
    }
}
