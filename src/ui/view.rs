use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use adw::gdk;
use adw::prelude::*;
use gtk::glib::{self};
use relm4::prelude::*;
use std::path::PathBuf;

#[relm4::component(pub, async)]
impl SimpleAsyncComponent for FluxApp {
    type Init = PathBuf;
    type Input = AppMsg;
    type Output = ();

    view! {
        /// Main application window for the Flux file manager.
        adw::Window {
            #[watch]
            set_title: Some(&if model.current_path.to_string_lossy().starts_with(constants::TRASH_URI) {
                tr("Trash")
            } else {
                model.current_path.file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("/"))
                    .to_string_lossy()
                    .into_owned()
            }),

            #[watch]
            set_decorated: model.config.ui.show_csd,

            // --- UI LAYOUT ---
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_hexpand: true,
                set_vexpand: true,

                /// Left sidebar for system places and user bookmarks.
                #[name = "sidebar_box"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_width_request: model.config.ui.sidebar_width,
                    add_css_class: constants::SIDEBAR_CSS_CLASS,
                    #[watch]
                    set_visible: model.sidebar_visible,

                    #[name = "sidebar_container"]
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                    },
                },

                /// Main content container for the header and file browser.
                #[local_ref]
                toast_overlay -> adw::ToastOverlay {
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

                                    #[local_ref]
                                    breadcrumb_box -> gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        add_css_class: "linked",
                                        set_spacing: 0,
                                        set_halign: gtk::Align::Center,
                                        set_valign: gtk::Align::Center,
                                        set_focusable: true,
                                        set_can_focus: true,
                                        connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),

                                        add_controller = gtk::EventControllerKey {
                                            set_propagation_phase: gtk::PropagationPhase::Capture,
                                            connect_key_pressed => move |ctrl, keyval, _, _| {
                                                let widget = ctrl.widget().unwrap();
                                                let b = widget.downcast_ref::<gtk::Box>().unwrap();
                                                // Walk children to find the one that currently owns GTK focus,
                                                // focus_child() is only a layout routing hint and may be stale.
                                                let mut focused: Option<gtk::Widget> = None;
                                                let mut child = b.first_child();
                                                while let Some(w) = child {
                                                    if w.has_focus() {
                                                        focused = Some(w.clone());
                                                        break;
                                                    }
                                                    child = w.next_sibling();
                                                }
                                                match keyval {
                                                    gdk::Key::Left => {
                                                        if let Some(prev) = focused.as_ref().and_then(|w| w.prev_sibling()) {
                                                            prev.grab_focus();
                                                        } else if let Some(last) = b.last_child() {
                                                            last.grab_focus();
                                                        }
                                                        glib::Propagation::Stop
                                                    }
                                                    gdk::Key::Right => {
                                                        if let Some(next) = focused.as_ref().and_then(|w| w.next_sibling()) {
                                                            next.grab_focus();
                                                        } else if let Some(first) = b.first_child() {
                                                            first.grab_focus();
                                                        }
                                                        glib::Propagation::Stop
                                                    }
                                                    gdk::Key::Home => {
                                                        if let Some(first) = b.first_child() {
                                                            first.grab_focus();
                                                        }
                                                        glib::Propagation::Stop
                                                    }
                                                    gdk::Key::End => {
                                                        if let Some(last) = b.last_child() {
                                                            last.grab_focus();
                                                        }
                                                        glib::Propagation::Stop
                                                    }
                                                    _ => glib::Propagation::Proceed,
                                                }
                                            }
                                        },
                                        add_controller = gtk::GestureClick {
                                            set_propagation_phase: gtk::PropagationPhase::Capture,
                                            connect_pressed[sender] => move |_, n_press, _, _| {
                                                if n_press == 2 {
                                                    sender.input(AppMsg::SwitchHeader(constants::VIEW_ENTRY.to_string()));
                                                }
                                            }
                                        },
                                        connect_map => |container| {
                                            if let Some(last) = container.last_child() {
                                                last.grab_focus();
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

                                    // Populate once when the entry is mapped (i.e. when the user
                                    // switches to path-entry mode), not on every model update.
                                    connect_map[current_path = model.current_path.clone()] => move |entry| {
                                        entry.set_text(&current_path.to_string_lossy());
                                        let pos = entry.text_length() as i32;
                                        entry.set_position(pos);
                                        entry.grab_focus();
                                    },
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
                                            // Use expand_path for consistent path normalization (handles ~, absolute paths, etc.)
                                            let normalized_path = crate::utils::expand_path(&path_str);
                                            sender.input(AppMsg::Navigate(normalized_path));
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
                                set_tooltip_text: Some(&tr("Empty Trash")),
                                add_css_class: constants::DESTRUCTIVE_ACTION_CLASS,
                                connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: constants::HEADER_BTN_SPACING,

                                    gtk::Image {
                                        set_icon_name: Some(constants::ICON_TRASH),
                                    },
                                    gtk::Label {
                                        set_label: &tr("Empty Trash"),
                                    }
                                }
                            },

                            // ── Contextual button for Recents ───────────────────────────────
                            pack_end = &gtk::Button {
                                #[watch]
                                set_visible: model.current_path.to_string_lossy() == constants::RECENT_URI,
                                connect_clicked => AppMsg::ClearRecents,
                                #[watch]
                                set_tooltip_text: Some(&model.recents_tooltip),
                                add_css_class: constants::DESTRUCTIVE_ACTION_CLASS,
                                connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: constants::HEADER_BTN_SPACING,

                                    gtk::Image {
                                        set_icon_name: Some("edit-clear-symbolic"),
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.recents_label,
                                    }
                                }
                            },

                            pack_end = &gtk::MenuButton {
                                set_icon_name: "open-menu-symbolic",
                                set_tooltip_text: Some(&tr("Main Menu")),
                                add_css_class: "flat",
                                connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),
                                #[wrap(Some)]
                                set_popover: main_menu_popover = &gtk::PopoverMenu::from_model(
                                    Option::<&gtk::gio::MenuModel>::None,
                                ) {
                                    set_has_arrow: false,
                                }
                            },

                            /// Visual indicator of the current active sorting mode.
                            pack_end = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: constants::STATUS_ICON_SPACING,
                                set_margin_end: constants::HEADER_MARGIN_END,
                                set_valign: gtk::Align::Center,
                                add_css_class: constants::SORT_CONTAINER_CLASS,

                                add_controller = gtk::GestureClick {
                                    // Claim the event on press to prevent double-click propagation
                                    set_propagation_phase: gtk::PropagationPhase::Capture,
                                    connect_pressed[sender] => move |gesture, _, _, _| {
                                        // Stop the event from propagating further (e.g., to window maximization logic)
                                        gesture.set_state(gtk::EventSequenceState::Claimed);
                                        sender.input(AppMsg::CycleSort);
                                    }
                                },

                                gtk::Image {
                                    set_icon_name: Some(constants::ICON_SORT_INDICATOR),
                                    set_pixel_size: constants::STATUS_ICON_SIZE,
                                    set_opacity: constants::OPACITY_ICON,
                                },
                                gtk::Label {
                                    add_css_class: constants::SORT_LABEL_CLASS,
                                    set_opacity: constants::OPACITY_LABEL,
                                    #[watch]
                                    set_label: &model.sort_status(),
                                }
                            },
                        },

                        /// Main scrollable viewport for the file grid.
                        #[name = "main_paned"]
                        gtk::Paned {
                            set_orientation: gtk::Orientation::Vertical,
                            set_vexpand: true,
                            set_wide_handle: true,
                            set_shrink_end_child: true,
                            set_shrink_start_child: true,

                            #[wrap(Some)]
                            set_start_child: grid_scroller = &gtk::ScrolledWindow {
                                set_vexpand: true,
                                /// Scroll event controller for UI zooming (Ctrl + Scroll).
                                add_controller = gtk::EventControllerScroll {
                                    set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                                    set_propagation_phase: gtk::PropagationPhase::Capture,
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
                            },
                            #[wrap(Some)]
                            set_end_child: terminal_revealer = &gtk::Revealer {
                                set_transition_type: gtk::RevealerTransitionType::SlideUp,
                                set_transition_duration: 150,
                                #[watch]
                                set_reveal_child: model.terminal_visible,
                                #[watch]
                                set_visible: model.terminal_visible,
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
                                },
                            },
                        },

                      /// Selection status bar at the bottom of the main content view.
                      gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_margin_all: 6,
                            set_spacing: 12,
                            add_css_class: "selection-status",

                            /// Visual indicator for background task completion
                            gtk::ProgressBar {
                                #[watch]
                                set_visible: model.task_queue.summary().is_some(),
                                #[watch]
                                set_fraction: model.task_queue.summary().map(|(_, _, pct)| pct).unwrap_or(0.0),
                                set_halign: gtk::Align::Start,
                                set_valign: gtk::Align::Center,
                                set_width_request: 150,
                            },

                            /// Cancel button, visible only while transfers are in flight.
                            gtk::Button {
                                #[watch]
                                set_visible: model.task_queue.summary().is_some(),
                                set_icon_name: "process-stop-symbolic",
                                set_tooltip_text: Some(&tr("Cancel all transfers")),
                                add_css_class: constants::DESTRUCTIVE_ACTION_CLASS,
                                set_valign: gtk::Align::Center,
                                connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),
                                connect_clicked => AppMsg::CancelAllTasks,
                            },

                            /// Selection information
                            gtk::Label {
                                #[watch]
                                set_label: &model.selection_status,
                                add_css_class: "caption",
                                set_halign: gtk::Align::End,
                                set_hexpand: true, // This pushes the label to the right
                            }
                        }
                    }
                }
            }
        }
    }

    async fn update(&mut self, message: Self::Input, sender: AsyncComponentSender<Self>) {
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
    async fn init(
        start_path: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (mut model, breadcrumb_box) = Self::init_components(start_path, &root, sender.clone());
        let toast_overlay = &model.toast_overlay;
        let widgets = view_output!();

        let main_menu = Self::build_main_menu();
        widgets.main_menu_popover.set_menu_model(Some(&main_menu));

        // Map widgets that were not part of the view macro directly
        widgets.grid_scroller.set_child(Some(&model.files.view));
        widgets
            .sidebar_container
            .set_child(Some(model.sidebar.widget()));
        model
            .context_menu_popover
            .set_parent(&widgets.grid_scroller);

        // Store sidebar widget reference for toggling
        model.sidebar_widget = Some(widgets.sidebar_box.clone().upcast());

        // Store the paned reference in the model
        model.terminal_paned = Some(widgets.main_paned.clone());

        if let Some(paned) = &model.terminal_paned {
            let sender_clone = sender.clone();
            paned.connect_notify(Some("position"), move |paned, _| {
                // Calculate terminal height from paned position
                let height = paned.height();
                let position = paned.position();
                let terminal_height = height - position;

                // Convert pixels to character lines (roughly divide by 24)
                let terminal_lines = terminal_height / 24;
                if terminal_lines > 0 {
                    sender_clone.input(AppMsg::SetTerminalHeight(terminal_lines));
                }
            });
        }

        crate::ui::inputs::setup_controllers(
            &root,
            &model.files.view,
            sender.clone(),
            &widgets.header_stack,
            model.config.ui.single_click,
            &model.keymap,
            &model.terminal.drawing_area,
        );

        root.set_default_size(
            model.config.ui.startup_window_width,
            model.config.ui.startup_window_height,
        );

        if model.config.ui.start_maximized {
            root.maximize();
        }

        // The terminal is already initialized in init_components
        // We just need to get the terminal widget from the model
        let terminal_widget = model.terminal.drawing_area.clone();
        let terminal_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        terminal_box.append(&terminal_widget);
        widgets.terminal_revealer.set_child(Some(&terminal_box));

        AsyncComponentParts { model, widgets }
    }
}
