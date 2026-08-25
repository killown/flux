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
    type Init = crate::model::AppInit;
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

            /// UI LAYOUT
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

                            // WARNING: If you remove #[watch] here, Ctrl+F and double-click
                            // on breadcrumbs will stop working. The stack won't switch to
                            // VIEW_SEARCH or VIEW_ENTRY. This binding is critical.
                            /// Multi-state title stack for Breadcrumbs, Path Entry, and Search modes.
                            #[wrap(Some)]
                                set_title_widget: header_stack = &gtk::Stack {
                                    set_halign: gtk::Align::Center,
                                    set_hexpand: false,
                                    set_width_request: constants::LOCATION_ENTRY_WIDTH_REQUEST,
                                    set_transition_type: gtk::StackTransitionType::Crossfade,
                                    #[watch]
                                    set_visible_child_name: &model.header_view,

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
                                #[name = "header_path_entry"]
                                add_child = &gtk::Entry {
                                    set_hexpand: false,
                                    set_halign: gtk::Align::Center,
                                    set_width_request: constants::LOCATION_ENTRY_WIDTH_REQUEST,
                                    set_max_width_chars: constants::BREADCRUMB_MAX_WIDTH_CHARS as i32,

                                    connect_map => |entry| {
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
                                            let normalized_path = crate::utils::expand_path(&path_str);
                                            sender.input(AppMsg::Navigate(normalized_path));
                                        }
                                        sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                    },
                                } -> { set_name: constants::VIEW_ENTRY },

                                /// Content search layout container
                                add_child = &gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: constants::HEADER_BTN_SPACING,
                                    set_halign: gtk::Align::Center,

                                    /// Filtering and search input field.
                                    append = &gtk::SearchEntry {
                                        set_hexpand: false,
                                        set_width_request: constants::SEARCH_ENTRY_WIDTH_REQUEST,
                                        #[track = "model.search_just_opened"]
                                        set_text: &model.filter,
                                        add_controller = gtk::EventControllerKey {
                                            connect_key_pressed[sender] => move |_, keyval, _, _| {
                                                if keyval == gdk::Key::Escape {
                                                    sender.input(AppMsg::CancelContentSearch);
                                                    sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                                    return glib::Propagation::Stop;
                                                }
                                                glib::Propagation::Proceed
                                            }
                                        },
                                        // Enter key triggers content search if it's a ':' query
                                        connect_activate[sender] => move |entry| {
                                            let raw_text = entry.text().to_string();
                                            if raw_text.starts_with(':') {
                                                if let Some((term, ext_filter)) = crate::utils::search::parse_content_search_query(&raw_text) {
                                                    sender.input(AppMsg::StartContentSearch(term, ext_filter));
                                                }
                                            } else {
                                                sender.input(AppMsg::UpdateFilter(raw_text));
                                            }
                                        },
                                        // Live search for standard filters (size, time, filenames)
                                        connect_search_changed[sender] => move |entry| {
                                            let text = entry.text().to_string();
                                            if !text.starts_with(':') {
                                                sender.input(AppMsg::UpdateFilter(text));
                                            }
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
                                            sender.input(AppMsg::CancelContentSearch);
                                            sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                        },
                                        add_controller = gtk::GestureClick {
                                            connect_pressed[sender] => move |_, _, _, _| {
                                                sender.input(AppMsg::SwitchHeader(constants::VIEW_ENTRY.to_string()));
                                            }
                                        },
                                    },
                                    /// Activity indicator for content search.
                                    append = &gtk::Spinner {
                                        #[watch]
                                        set_spinning: model.is_content_searching,
                                        #[watch]
                                        set_visible: model.is_content_searching,
                                    },

                                    /// Cancellation button for content search operations.
                                    append = &gtk::Button {
                                        set_icon_name: "process-stop-symbolic",
                                        add_css_class: constants::DESTRUCTIVE_ACTION_CLASS,
                                        #[watch]
                                        set_visible: model.is_content_searching,
                                        connect_clicked => AppMsg::CancelContentSearch,
                                    },
                                } -> { set_name: constants::VIEW_SEARCH },

                                // Filter entry child, user types one or more glob patterns separated by comma.
                                // Committing (Enter) or pressing Escape exits back to VIEW_PATH.
                                add_child = &gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: constants::HEADER_BTN_SPACING,
                                    set_halign: gtk::Align::Center,

                                    append = &gtk::Entry {
                                        set_hexpand: false,
                                        set_halign: gtk::Align::Center,
                                        set_width_request: constants::SEARCH_ENTRY_WIDTH_REQUEST,
                                        set_placeholder_text: Some(&tr("Patterns: *.py, image/*, audio/*")),
                                        set_secondary_icon_name: Some("edit-clear-symbolic"),
                                        set_secondary_icon_tooltip_text: Some(&tr("Clear filter")),

                                        connect_map => |e| { e.grab_focus(); },

                                        connect_icon_press[sender] => move |_, pos| {
                                            if pos == gtk::EntryIconPosition::Secondary {
                                                sender.input(AppMsg::ClearExtensionFilter);
                                                sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                            }
                                        },

                                        connect_activate[sender] => move |entry| {
                                            let raw = entry.text().to_string();
                                            let patterns: Vec<String> = raw
                                                .split(',')
                                                .map(|p| p.trim().to_lowercase())
                                                .filter(|p| !p.is_empty())
                                                .collect();
                                            if patterns.is_empty() {
                                                sender.input(AppMsg::ClearExtensionFilter);
                                            } else {
                                                sender.input(AppMsg::SetExtensionFilter(patterns));
                                            }
                                            sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
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
                                    },
                                } -> { set_name: constants::VIEW_FILTER },
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

                            /// Contextual button for Recents
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

                            /// Toggle between grid card layout and compact list layout.
                            pack_end = &gtk::ToggleButton {
                                #[watch]
                                set_active: model.is_list_mode,
                                set_icon_name: "view-list-symbolic",
                                set_tooltip_text: Some(&tr("Toggle List Mode")),
                                add_css_class: "flat",
                                connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),
                                connect_clicked => AppMsg::ToggleListMode,
                            },

                            // Filter toggle button, opens the VIEW_FILTER stack child.
                            // Active state tracks whether any patterns are set.
                            pack_end = &gtk::ToggleButton {
                                set_icon_name: constants::ICON_FILTER,
                                set_tooltip_text: Some(&tr("Filter by Pattern")),
                                add_css_class: "flat",
                                #[watch]
                                set_active: model.extension_filter.is_some(),
                                connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),
                                connect_clicked[sender] => move |btn| {
                                    if btn.is_active() {
                                        sender.input(AppMsg::SwitchHeader(constants::VIEW_FILTER.to_string()));
                                    } else {
                                        sender.input(AppMsg::ClearExtensionFilter);
                                        sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
                                    }
                                },
                            },

                            /// Visual indicator of the current active sorting mode.
                            pack_end = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: constants::STATUS_ICON_SPACING,
                                set_margin_end: constants::HEADER_MARGIN_END,
                                set_valign: gtk::Align::Center,
                                add_css_class: constants::SORT_CONTAINER_CLASS,

                                add_controller = gtk::GestureClick {
                                    set_propagation_phase: gtk::PropagationPhase::Capture,
                                    connect_pressed[sender] => move |gesture, _, _, _| {
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

                        // Filter chip bar, slides in below the header when patterns are active.
                        gtk::Revealer {
                            set_transition_type: gtk::RevealerTransitionType::SlideDown,
                            set_transition_duration: 150,
                            #[watch]
                            set_reveal_child: model.extension_filter.is_some(),
                            #[watch]
                            set_visible: model.extension_filter.is_some(),

                            #[name = "filter_bar"]
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_margin_start: 8,
                                set_margin_end: 8,
                                set_margin_top: 4,
                                set_margin_bottom: 4,
                                add_css_class: constants::FILTER_BAR_CSS_CLASS,

                                gtk::Label {
                                    set_label: &tr("Filter:"),
                                    add_css_class: "caption",
                                    set_opacity: 0.6,
                                },
                                // Chips are built imperatively in update.rs via rebuild_filter_bar()
                                // whenever SetExtensionFilter / ClearExtensionFilter is handled.
                            }
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
                            set_start_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_vexpand: true,

                                #[name = "grid_overlay"]
                                gtk::Overlay {
                                    set_vexpand: true,

                                    #[name = "grid_scroller"]
                                    gtk::ScrolledWindow {
                                        set_vexpand: true,
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Fill,
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

                                    /// Lock overlay shown when the current archive requires a password.
                                    add_overlay = &gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_halign: gtk::Align::Center,
                                        set_valign: gtk::Align::Center,
                                        set_spacing: 12,
                                        set_margin_all: 24,
                                        set_can_target: false,
                                        #[watch]
                                        set_visible: model.archive_locked,

                                        gtk::Image {
                                            set_icon_name: Some("changes-prevent-symbolic"),
                                            set_pixel_size: 64,
                                            add_css_class: "dim-label",
                                        },

                                        gtk::Label {
                                            set_label: &crate::i18n::tr("Archive is password-protected"),
                                            add_css_class: "title-3",
                                        },

                                        gtk::Label {
                                            set_label: &crate::i18n::tr("Enter the password to browse its contents."),
                                            add_css_class: "dim-label",
                                        },
                                    },

                                    /// Loading overlay spinner for slow directories/archives.
                                    add_overlay = &gtk::Spinner {
                                        set_halign: gtk::Align::Center,
                                        set_valign: gtk::Align::Center,
                                        set_size_request: (48, 48),
                                        #[watch]
                                        set_spinning: model.is_loading,
                                        #[watch]
                                        set_visible: model.is_loading,
                                    },
                                },

                                gtk::Revealer {
                                    set_transition_type: gtk::RevealerTransitionType::SlideUp,
                                    set_transition_duration: 150,
                                    #[watch]
                                    set_reveal_child: !model.exclusive_list.is_empty(),
                                    #[watch]
                                    set_visible: !model.exclusive_list.is_empty(),

                                    gtk::ScrolledWindow {
                                        set_hscrollbar_policy: gtk::PolicyType::Automatic,
                                        set_vscrollbar_policy: gtk::PolicyType::Never,
                                        set_propagate_natural_height: true,
                                        add_css_class: "quick-panel-scroll",

                                        #[local_ref]
                                        quick_panel_box -> gtk::Box {
                                            set_orientation: gtk::Orientation::Horizontal,
                                            set_halign: gtk::Align::Center,
                                            add_css_class: "quick-panel",
                                            set_spacing: 4,
                                            set_margin_start: 6,
                                            set_margin_end: 6,
                                            set_margin_top: 6,
                                            set_margin_bottom: 6,
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
                            add_controller = gtk::DropTarget {
                                set_types: &[gdk::FileList::static_type()],
                                set_actions: gdk::DragAction::MOVE | gdk::DragAction::COPY,

                                connect_drop[sender, current_path = model.current_path.clone()] => move |gesture, value, x, y| {
                                    if let Ok(file_list) = value.get::<gdk::FileList>() {
                                        let source_paths: Vec<PathBuf> = file_list.files()
                                            .iter()
                                            .map(|f| f.path().unwrap_or_default())
                                            .collect();

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

                            /// ─── Show Progress button ────────────────────────────────
                            gtk::Button {
                                #[watch]
                                set_visible: model.show_transfer_button(),
                                set_icon_name: "dialog-information-symbolic",
                                set_label: crate::i18n::tr(" Show Progress").as_str(),
                                set_tooltip_text: Some(crate::i18n::tr("Open transfer progress dialog").as_str()),
                                add_css_class: "flat",
                                connect_clicked => AppMsg::ShowTransferDialog,
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
                                set_hexpand: true,
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
    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let crate::model::AppInit {
            start_path,
            open_archive,
            quick_list,
        } = init;
        let (mut model, breadcrumb_box) =
            Self::init_components(start_path, quick_list, &root, sender.clone());
        let toast_overlay = &model.toast_overlay;
        let quick_panel_box = model.quick_panel_box.clone();
        let widgets = view_output!();

        let main_menu = Self::build_main_menu();
        widgets.main_menu_popover.set_menu_model(Some(&main_menu));
        model.header_path_entry = widgets.header_path_entry.downgrade();

        widgets.grid_scroller.set_child(Some(&model.files.view));

        let sidebar_wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        if model.sidebar.widget().parent().is_none() {
            sidebar_wrapper.append(model.sidebar.widget());
        }
        sidebar_wrapper.append(&model.network_section);
        widgets.sidebar_container.set_child(Some(&sidebar_wrapper));

        let pin_zone = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(8)
            .css_classes(["sidebar-pin-zone"])
            .visible(false)
            .build();
        let pin_icon = gtk::Image::builder().icon_name("list-add-symbolic").build();
        let pin_label = gtk::Label::builder()
            .label(tr("Pin to Sidebar"))
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();
        pin_label.add_css_class("sidebar-pin-zone-label");
        pin_zone.append(&pin_icon);
        pin_zone.append(&pin_label);
        // prepend into sidebar_box (outside the ScrolledWindow) so it always
        // appears at the very top, above the scrollable bookmark list.
        widgets.sidebar_box.prepend(&pin_zone);

        {
            let sidebar_ft = gtk::DropTarget::builder()
                .actions(gdk::DragAction::COPY | gdk::DragAction::MOVE)
                .preload(false)
                .build();
            sidebar_ft.set_types(&[gdk::FileList::static_type()]);
            let pz_enter = pin_zone.clone();
            sidebar_ft.connect_enter(move |_, _, _| {
                pz_enter.set_visible(true);
                gdk::DragAction::empty()
            });
            let pz_leave = pin_zone.clone();
            sidebar_ft.connect_leave(move |_| {
                let pz = pz_leave.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
                    pz.set_visible(false);
                });
            });
            widgets.sidebar_box.add_controller(sidebar_ft);
        }

        {
            let pin_zone_dt = gtk::DropTarget::builder()
                .actions(gdk::DragAction::COPY | gdk::DragAction::MOVE)
                .build();
            pin_zone_dt.set_types(&[gdk::FileList::static_type()]);
            let pz_hover = pin_zone.clone();
            pin_zone_dt.connect_enter(move |_, _, _| {
                pz_hover.add_css_class("sidebar-pin-zone-hover");
                gdk::DragAction::COPY
            });
            let pz_leave2 = pin_zone.clone();
            pin_zone_dt.connect_leave(move |_| {
                pz_leave2.remove_css_class("sidebar-pin-zone-hover");
            });
            let s_pin = sender.clone();
            let pz_drop = pin_zone.clone();
            pin_zone_dt.connect_drop(move |_, value, _, _| {
                pz_drop.remove_css_class("sidebar-pin-zone-hover");
                pz_drop.set_visible(false);
                if let Ok(file_list) = value.get::<gdk::FileList>() {
                    let paths: Vec<std::path::PathBuf> = file_list
                        .files()
                        .into_iter()
                        .filter_map(|f| f.path())
                        .filter(|p| p.is_dir())
                        .collect();
                    for folder in paths {
                        s_pin.input(AppMsg::PinFolderAt {
                            path: folder,
                            before: std::path::PathBuf::new(),
                            label_name: None,
                        });
                    }
                    return true;
                }
                false
            });
            pin_zone.add_controller(pin_zone_dt);
        }

        model.context_menu_popover.set_parent(&widgets.grid_overlay);
        model.sidebar_widget = Some(widgets.sidebar_box.clone().upcast());

        model.terminal_paned = Some(widgets.main_paned.clone());

        if let Some(paned) = &model.terminal_paned {
            let sender_clone = sender.clone();
            paned.connect_notify(Some("position"), move |paned, _| {
                let total_height = paned.height();
                if total_height == 0 {
                    return;
                }
                let terminal_px = total_height - paned.position();
                if terminal_px > 50 {
                    sender_clone.input(AppMsg::SetTerminalHeight(terminal_px));
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

        let terminal_widget = model.terminal.drawing_area.clone();
        let terminal_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        terminal_box.append(&terminal_widget);
        widgets.terminal_revealer.set_child(Some(&terminal_box));

        if let Some(archive_path) = open_archive {
            let s = sender.clone();
            gtk::glib::idle_add_local_once(move || {
                s.input(AppMsg::EnterArchive(archive_path));
            });
        }

        AsyncComponentParts { model, widgets }
    }
}
