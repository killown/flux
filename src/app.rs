use adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::model::{AppMsg, FluxApp};
use crate::ui::{constants, FileItem, SidebarPlace};
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
        /// Main application window for the Flux file manager.
        adw::Window {
            set_default_size: (constants::DEFAULT_WIDTH, constants::DEFAULT_HEIGHT),
            set_title: Some(constants::APP_TITLE),

            // --- INPUT CONTROLLERS ---

            /// Primary key handler for standard navigation and system shortcuts.
            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender, header_view = model.header_view.clone()] => move |ctrl, keyval, _, state| {
                     FluxApp::handle_key_event(ctrl, keyval, state, &sender, &header_view)
                }
            },

            /// Capture-phase controller for exclusive mode management and specialized list navigation.
            add_controller = gtk::EventControllerKey {
                set_propagation_phase: gtk::PropagationPhase::Capture,
                connect_key_pressed[sender] => move |_ctrl, keyval, _keycode, state| {
                    let modifiers = state & gtk::accelerator_get_default_mod_mask();
                    let is_ctrl = modifiers == gdk::ModifierType::CONTROL_MASK;

                    match keyval {
                        gdk::Key::Insert => {
                            sender.input(AppMsg::AddExclusive);
                            glib::Propagation::Stop
                        }
                        gdk::Key::End if is_ctrl => {
                            sender.input(AppMsg::ClearExclusive);
                            glib::Propagation::Stop
                        }
                        gdk::Key::Page_Up if is_ctrl => {
                            sender.input(AppMsg::PrevExclusive);
                            glib::Propagation::Stop
                        }
                        gdk::Key::Page_Down if is_ctrl => {
                            sender.input(AppMsg::NextExclusive);
                            glib::Propagation::Stop
                        }
                        gdk::Key::Tab => {
                            sender.input(AppMsg::NextExclusive);
                            glib::Propagation::Stop
                        }
                        _ => glib::Propagation::Proceed,
                    }
                }
            },

            /// dedicated history navigation controller for bracket-based folder movement.
            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender] => move |_, keyval, _, state| {
                    let modifiers = state & gtk::accelerator_get_default_mod_mask();
                    let is_ctrl = modifiers == gdk::ModifierType::CONTROL_MASK;
                    match keyval {
                        gdk::Key::bracketleft if is_ctrl => {
                            sender.input(AppMsg::CycleRecent(-1));
                            glib::Propagation::Stop
                        }
                        gdk::Key::bracketright if is_ctrl => {
                            sender.input(AppMsg::CycleRecent(1));
                            glib::Propagation::Stop
                        }
                        _ => glib::Propagation::Proceed
                    }
                }
            },

            /// Mouse button handler for auxiliary navigation controls (Buttons 8/9).
            add_controller = gtk::GestureClick {
                set_button: 0,
                connect_pressed => |gesture, _, _, _| {
                    let button = gesture.current_button();
                    if button == constants::MOUSE_BACK || button == constants::MOUSE_FORWARD {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                    }
                },
                connect_released[sender] => move |gesture, _, _, _| {
                    let button = gesture.current_button();
                    let state = gesture.current_event_state();
                    let modifiers = state & gtk::accelerator_get_default_mod_mask();

                    if button == constants::MOUSE_BACK && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                        sender.input(AppMsg::JumpToRecent(0));
                    }
                }
            },

            // --- UI LAYOUT ---

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

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
                        set_show_start_title_buttons: false,
                        set_show_end_title_buttons: false,

                        pack_start = &gtk::Button {
                            set_icon_name: constants::ICON_BACK,
                            connect_clicked => AppMsg::GoBack,
                            #[watch] set_sensitive: !model.history.is_empty(),
                        },
                        pack_start = &gtk::Button {
                            set_icon_name: constants::ICON_FORWARD,
                            connect_clicked => AppMsg::GoForward,
                            #[watch] set_sensitive: !model.forward_stack.is_empty(),
                        },

                        /// Multi-state title stack for Breadcrumbs, Path Entry, and Search modes.
                        #[wrap(Some)]
                        set_title_widget = &gtk::Stack {
                            #[watch] set_visible_child_name: &model.header_view,
                            set_transition_type: gtk::StackTransitionType::Crossfade,

                            /// Current path display; triggers editable entry mode on click.
                            add_child = &gtk::Button {
                                add_css_class: "flat",
                                #[watch] set_label: &model.current_path.to_string_lossy(),
                                connect_clicked => AppMsg::SwitchHeader(constants::VIEW_ENTRY.to_string()),
                            } -> { set_name: "path_old" },

                            /// Interactive breadcrumb container for directory parent navigation.
                            #[name = "path_entry"]
                            add_child = &gtk::ScrolledWindow {
                                set_hscrollbar_policy: gtk::PolicyType::External,
                                set_vscrollbar_policy: gtk::PolicyType::Never,
                                set_halign: gtk::Align::Center,
                                set_min_content_width: constants::SCROLLED_WINDOW_MIN_WIDTH,

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
        // Global static access for background thread communication
        let _ = crate::model::SENDER.set(sender.input_sender().clone());
        relm4::set_global_css(include_str!("style.css"));

        // Setup window-level shortcut controllers
        let shortcut_controller = gtk::ShortcutController::new();
        Self::setup_shortcuts(&shortcut_controller, &sender);
        root.add_controller(shortcut_controller);

        let config = utils::load_config();
        let menu_actions_list = utils::load_menu_config();
        let context_menu_popover = gtk::PopoverMenu::builder().has_arrow(false).build();

        // Initialize and register the GAction group for window-scoped actions
        let action_group = gio::SimpleActionGroup::new();
        root.insert_action_group("win", Some(&action_group));

        // Configure the main file grid view
        let files = TypedGridView::<FileItem, gtk::MultiSelection>::new();
        files.view.set_enable_rubberband(true);
        files.view.set_single_click_activate(config.ui.single_click);

        let grid_view = &files.view;
        let sender_clone = sender.clone();
        grid_view.connect_activate(move |_, _| sender_clone.input(AppMsg::Open));

        // Setup the sidebar list with navigation forwarding
        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder()
            .launch(listbox)
            .forward(sender.input_sender(), AppMsg::Navigate);

        // Monitor external volume changes to keep sidebar drive list in sync
        let volume_monitor = gio::VolumeMonitor::get();
        let s_added = sender.clone();
        volume_monitor.connect_mount_added(move |_, _| s_added.input(AppMsg::RefreshSidebar));

        let s = sender.clone();
        volume_monitor.connect_mount_removed(move |_, _| {
            s.input(AppMsg::RefreshSidebar);
        });

        // Setup breadcrumb navigation bar
        let breadcrumb_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let breadcrumbs = FactoryVecDeque::builder()
            .launch(breadcrumb_box.clone())
            .forward(sender.input_sender(), AppMsg::Navigate);

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
            exclusive_list: Vec::new(),
            exclusive_index: None,
            search_just_opened: false,
            sort_by: config.ui.default_sort.clone(),
            show_hidden: config.ui.show_hidden_by_default,
            config: config.clone(),
            _volume_monitor: volume_monitor,
            filter: String::new(),
            header_view: constants::VIEW_PATH.to_string(),
            recent_stack: std::collections::VecDeque::with_capacity(
                constants::RECENT_STACK_CAPACITY,
            ),
        };

        model.setup_actions(&sender);
        model.recent_stack.push_front(start_path.clone());
        model.update_breadcrumbs();

        // Populate sidebar from persistent configuration
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

        // Final widget-to-layout assembly
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
