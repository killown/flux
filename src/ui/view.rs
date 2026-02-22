use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use adw::gdk;
use adw::prelude::*;
use gtk::glib::{self};
use relm4::prelude::*;
use std::path::PathBuf;

#[relm4::component(pub)]
impl SimpleComponent for FluxApp {
    type Init = PathBuf;
    type Input = AppMsg;
    type Output = ();

    view! {
        /// Main application window for the Flux file manager.
        adw::Window {
            set_default_width: constants::DEFAULT_WIDTH,
            set_default_height: constants::DEFAULT_HEIGHT,
            set_title: Some(constants::APP_TITLE),


            #[watch]
            set_decorated: model.config.ui.show_csd,

            // --- UI LAYOUT ---
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_hexpand: true,
                set_vexpand: true,

                /// Left sidebar for system places and user bookmarks.
                #[name = "sidebar_container"]
                gtk::ScrolledWindow {
                    set_width_request: model.config.ui.sidebar_width,
                    add_css_class: constants::SIDEBAR_CSS_CLASS,
                },

                /// Main content container for the header and file browser.
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,

                    /// Top-level navigation and state toolbar.
                    adw::HeaderBar {
                        #[watch]
                        set_show_start_title_buttons: model.config.ui.show_csd,
                        #[watch]
                        set_show_end_title_buttons: model.config.ui.show_csd,

                        pack_start = &gtk::Button {
                            set_icon_name: constants::ICON_BACK,
                            connect_clicked => AppMsg::GoBack,
                            #[watch] set_sensitive: !model.history.is_empty(),
                            connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),
                        },
                        pack_start = &gtk::Button {
                            set_icon_name: constants::ICON_FORWARD,
                            connect_clicked => AppMsg::GoForward,
                            #[watch] set_sensitive: !model.forward_stack.is_empty(),
                            connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),

                        },

                        /// Multi-state title stack for Breadcrumbs, Path Entry, and Search modes.
                        #[wrap(Some)]
                        set_title_widget: header_stack = &gtk::Stack {
                            set_halign: gtk::Align::Center,
                            set_hexpand: false,
                            set_width_request: constants::LOCATION_ENTRY_WIDTH_REQUEST,
                            #[watch] set_visible_child_name: &model.header_view,
                            set_transition_type: gtk::StackTransitionType::Crossfade,
                            add_child = &gtk::Button {
                                add_css_class: "flat",
                                #[watch] set_label: &model.current_path.to_string_lossy(),
                                connect_clicked => AppMsg::SwitchHeader(constants::VIEW_ENTRY.to_string()),
                            } -> { set_name: "path_old" },

                            /// Interactive breadcrumb container for directory parent navigation.
                            #[name = "path_entry"]
                            add_child = &gtk::ScrolledWindow {
                                set_propagate_natural_width: false,
                                set_min_content_width: constants::SCROLLED_WINDOW_MIN_WIDTH,
                                set_hscrollbar_policy: gtk::PolicyType::External,
                                set_vscrollbar_policy: gtk::PolicyType::Never,
                                set_halign: gtk::Align::Center,

                                gtk::Box {
                                    add_css_class: "linked",
                                    set_spacing: 0,
                                    set_halign: gtk::Align::Center,
                                    set_valign: gtk::Align::Center,
                                    #[local_ref]
                                    breadcrumb_box -> gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),

                                    },
                                    add_controller = gtk::GestureClick {
                                        set_propagation_phase: gtk::PropagationPhase::Capture,
                                        connect_pressed[sender] => move |_, n_press, _, _| {
                                            if n_press == 2 {
                                                sender.input(AppMsg::SwitchHeader(constants::VIEW_ENTRY.to_string()));
                                            }
                                        }
                                    },
                                }
                            } -> { set_name: constants::VIEW_PATH },

                            /// Direct path entry field for manual location input.
                            add_child = &gtk::Entry {
                                set_hexpand: false,
                                set_halign: gtk::Align::Center,
                                set_width_request: constants::LOCATION_ENTRY_WIDTH_REQUEST,
                                set_max_width_chars: constants::BREADCRUMB_MAX_WIDTH_CHARS as i32,
                                set_max_width_chars: constants::BREADCRUMB_MAX_WIDTH_CHARS as i32,

                                #[watch] set_text: &model.current_path.to_string_lossy(),
                                add_controller = gtk::EventControllerKey {
                                    connect_key_pressed[sender] => move |_, keyval, _, _| {
                                        if keyval == gdk::Key::Escape {
                                            sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
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
                                    sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                },
                            } -> { set_name: constants::VIEW_ENTRY },

                            /// Filtering and search input field.
                            add_child = &gtk::SearchEntry {
                                set_hexpand: false,
                                set_halign: gtk::Align::Center,
                                set_width_request: constants::SEARCH_ENTRY_WIDTH_REQUEST,
                                #[track = "model.search_just_opened"]
                                set_text: &model.filter,
                                add_controller = gtk::EventControllerKey {
                                    connect_key_pressed[sender] => move |_, keyval, _, _| {
                                        if keyval == gdk::Key::Escape {
                                            sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                            return glib::Propagation::Stop;
                                        }
                                        glib::Propagation::Proceed
                                    }
                                },
                                connect_activate[sender] => move |_| {
                                  sender.input(AppMsg::Activate);
                                },
                                connect_search_changed[sender] => move |entry| {
                                    sender.input(AppMsg::UpdateFilter(entry.text().to_string()));
                                },
                                connect_map[sender] => move |e| {
                                    e.grab_focus();
                                    let e_ptr = e.clone();
                                    let s_clone = sender.clone();
                                    glib::idle_add_local_once(move || {
                                        let pos = e_ptr.text().chars().count() as i32;
                                        e_ptr.set_position(pos);
                                        e_ptr.select_region(pos, pos);
                                        s_clone.input(AppMsg::CloseSearchSync);
                                    });
                                },
                                connect_stop_search[sender] => move |_| {
                                    sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                },
                                add_controller = gtk::GestureClick {
                                    connect_pressed[sender] => move |_, _, _, _| {
                                        sender.input(AppMsg::SwitchHeader(constants::VIEW_ENTRY.to_string()));
                                    }
                                },
                            } -> { set_name: constants::VIEW_SEARCH },
                        },

                        /// Destructive action button to purge the Trash directory.
                        pack_end = &gtk::Button {
                            #[watch]
                            set_visible: model.current_path.to_string_lossy() == constants::TRASH_URI,
                            connect_clicked => AppMsg::EmptyTrash,
                            set_tooltip_text: Some(constants::LABEL_EMPTY_TRASH),
                            add_css_class: constants::DESTRUCTIVE_ACTION_CLASS,
                            connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: constants::HEADER_BTN_SPACING,

                                gtk::Image {
                                    set_icon_name: Some(constants::ICON_TRASH),
                                },
                                gtk::Label {
                                    set_label: constants::LABEL_EMPTY_TRASH,
                                }
                            }
                        },
                        /// Visual indicator of the current active sorting mode.
                        pack_end = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: constants::STATUS_ICON_SPACING,
                            set_margin_end: constants::HEADER_MARGIN_END,
                            set_valign: gtk::Align::Center,
                            add_css_class: constants::SORT_CONTAINER_CLASS,

                            gtk::Image {
                                set_icon_name: Some(constants::ICON_SORT_INDICATOR),
                                set_pixel_size: constants::STATUS_ICON_SIZE,
                                set_opacity: constants::OPACITY_ICON,
                            },
                            gtk::Label {
                                add_css_class: constants::SORT_LABEL_CLASS,
                                set_opacity: constants::OPACITY_LABEL,
                                #[watch]
                                set_label: model.sort_status(),
                            }
                        }
                    },

                    /// Main scrollable viewport for the file grid.
                    #[name = "grid_scroller"]
                    gtk::ScrolledWindow {
                        set_vexpand: true,

                        /// Scroll event controller for UI zooming (Ctrl + Scroll).
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

                        /// Global drop handler for cross-instance and external file transfers.
                        ///
                        /// Listens for `text/uri-list` data (via `gdk::FileList`) to bridge independent
                        /// application processes that do not share a memory space.
                        add_controller = gtk::DropTarget {
                            set_types: &[gdk::FileList::static_type()],
                            set_actions: gdk::DragAction::MOVE | gdk::DragAction::COPY,

                            connect_drop[sender, current_path = model.current_path.clone()] => move |gesture, value, x, y| {
                                // 1. Force extraction of the file list
                                if let Ok(file_list) = value.get::<gdk::FileList>() {
                                    let source_paths: Vec<PathBuf> = file_list.files()
                                        .iter()
                                        .map(|f| f.path().unwrap_or_default())
                                        .collect();

                                    // 2. Logic to determine destination
                                    let mut dest_path = current_path.clone();
                                    if let Some(widget) = gesture.widget() {
                                        if let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) {
                                            let mut curr: Option<gtk::Widget> = Some(picked);
                                            while let Some(w) = curr {
                                                let name = w.widget_name().to_string();
                                                if name.starts_with('/') {
                                                    let p = PathBuf::from(name);
                                                    if p.is_dir() { dest_path = p; }
                                                    break;
                                                }
                                                curr = w.parent();
                                            }
                                        }
                                    }

                                    // 3. Trigger the external drop handler
                                    sender.input(AppMsg::HandleExternalDrop { source_paths, dest_path });
                                    return true;
                                }
                                false
                            }
                        },

                        /// Secondary click controller for context menu spawning.
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

    /// Initializes the Flux application component, setting up state, UI widgets, and system monitors.
    ///
    /// Args:
    ///     start_path: The initial filesystem path to load upon startup.
    ///     root: The root widget of the component.
    ///     sender: The communication channel for sending messages to the component.
    ///
    /// Returns:
    ///     The initialized model and widgets encapsulated in `ComponentParts`.
    fn init(
        start_path: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (model, breadcrumb_box) = Self::init_components(start_path, &root, sender.clone());
        let widgets = view_output!();

        // Map widgets that were not part of the view macro directly
        widgets.grid_scroller.set_child(Some(&model.files.view));
        widgets
            .sidebar_container
            .set_child(Some(model.sidebar.widget()));
        model
            .context_menu_popover
            .set_parent(&widgets.grid_scroller);

        crate::ui::inputs::setup_controllers(
            &root,
            &model.files.view,
            sender.clone(),
            &widgets.header_stack,
            model.config.ui.single_click,
            &model.keymap,
        );

        if model.config.ui.start_maximized {
            root.maximize();
        }

        root.connect_maximized_notify(move |window| {
            sender.input(AppMsg::SetMaximized(window.is_maximized()));
        });

        ComponentParts { model, widgets }
    }
}
