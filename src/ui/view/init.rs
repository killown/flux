use super::component::FluxAppWidgets;
use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use adw::gdk;
use adw::prelude::*;
use gtk::gio;
use gtk::glib::{self};
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Attaches the tab bar, header widget, main menu and sidebar wrapper.
    pub(super) fn attach_tabs_and_header(
        model: &mut FluxApp,
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        widgets.toolbar_view.add_top_bar(&model.tab_bar);
        widgets.tab_view_container.append(&model.tab_view);
        if let Some(first_tab) = model.tabs.first() {
            Self::connect_tab_scroller(&first_tab.scroller, sender);
        }

        model.header_widget = Some(widgets.header_bar.clone().upcast());

        let main_menu = Self::build_main_menu();
        widgets.main_menu_popover.set_menu_model(Some(&main_menu));
        model.header_path_entry = widgets.header_path_entry.downgrade();

        let sidebar_wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        if model.sidebar.widget().parent().is_none() {
            sidebar_wrapper.append(model.sidebar.widget());
        }
        sidebar_wrapper.append(&model.network_section);
        widgets.sidebar_container.set_child(Some(&sidebar_wrapper));
    }

    /// Wires the sidebar resize handle (hover cursor and drag gesture).
    pub(super) fn setup_sidebar_resize(
        model: &FluxApp,
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        {
            let sidebar_box_weak = widgets.sidebar_box.downgrade();
            let start_width = std::rc::Rc::new(std::cell::Cell::new(
                model.config.ui.sidebar_width.clamp(160, 500),
            ));
            let start_root_x = std::rc::Rc::new(std::cell::Cell::new(0.0));
            let drag_gesture = gtk::GestureDrag::new();
            let hover_timer = std::rc::Rc::new(std::cell::Cell::new(None::<glib::SourceId>));
            let is_ready = std::rc::Rc::new(std::cell::Cell::new(false));

            let motion_ctrl = gtk::EventControllerMotion::new();

            {
                let timer_c = hover_timer.clone();
                let ready_c = is_ready.clone();
                motion_ctrl.connect_enter(move |ctrl, _, _| {
                    if let Some(id) = timer_c.take() {
                        id.remove();
                    }
                    ready_c.set(false);

                    let ctrl_weak = ctrl.downgrade();
                    let timer_inner = timer_c.clone();
                    let ready_inner = ready_c.clone();

                    let id = glib::timeout_add_local_once(
                        std::time::Duration::from_millis(150),
                        move || {
                            timer_inner.set(None);
                            ready_inner.set(true);
                            if let Some(c) = ctrl_weak.upgrade() {
                                if let Some(widget) = c.widget() {
                                    widget.set_cursor_from_name(Some("col-resize"));
                                }
                            }
                        },
                    );
                    timer_c.set(Some(id));
                });
            }

            {
                let timer_c = hover_timer;
                let ready_c = is_ready.clone();
                motion_ctrl.connect_leave(move |ctrl| {
                    if let Some(id) = timer_c.take() {
                        id.remove();
                    }
                    ready_c.set(false);
                    if let Some(widget) = ctrl.widget() {
                        widget.set_cursor(None);
                    }
                });
            }

            widgets.sidebar_resize_handle.add_controller(motion_ctrl);

            {
                let s_box_w = sidebar_box_weak.clone();
                let start_w = start_width.clone();
                let start_rx = start_root_x.clone();
                let ready_c = is_ready.clone();

                drag_gesture.connect_drag_begin(move |gesture, x, _| {
                    if !ready_c.get() {
                        gesture.set_state(gtk::EventSequenceState::Denied);
                        return;
                    }
                    if let Some(s) = s_box_w.upgrade() {
                        start_w.set(s.width());
                        if let Some(root_win) = s.root() {
                            if let Some(handle) = gesture.widget() {
                                let (rx, _) = handle
                                    .translate_coordinates(&root_win, x, 0.0)
                                    .unwrap_or((x, 0.0));
                                start_rx.set(rx);
                            }
                        }
                    }
                });
            }

            {
                let s_box_w = sidebar_box_weak.clone();
                let start_w = start_width.clone();
                let start_rx = start_root_x.clone();
                let ready_c = is_ready.clone();

                drag_gesture.connect_drag_update(move |gesture, _, _| {
                    if !ready_c.get() {
                        return;
                    }
                    if let Some(s) = s_box_w.upgrade() {
                        if let Some(root_win) = s.root() {
                            if let Some(handle) = gesture.widget() {
                                if let Some((curr_x, _)) = gesture.point(None) {
                                    if let Some((curr_root_x, _)) =
                                        handle.translate_coordinates(&root_win, curr_x, 0.0)
                                    {
                                        let delta_x = curr_root_x - start_rx.get();
                                        let new_w =
                                            (start_w.get() + delta_x as i32).clamp(160, 500);

                                        if s.width_request() != new_w {
                                            s.set_width_request(new_w);
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }

            {
                let s_box_w = sidebar_box_weak;
                let s_sender = sender.clone();
                let ready_c = is_ready;

                drag_gesture.connect_drag_end(move |gesture, _, _| {
                    if !ready_c.get() {
                        return;
                    }
                    if let Some(s) = s_box_w.upgrade() {
                        let final_width = s.width().clamp(160, 500);
                        s.set_width_request(final_width);
                        s_sender.input(AppMsg::SetSidebarWidth(final_width));
                    }
                    if let Some(widget) = gesture.widget() {
                        widget.set_cursor(None);
                    }
                });
            }

            widgets.sidebar_resize_handle.add_controller(drag_gesture);
        }
    }

    /// Builds the "Pin to Sidebar" drop zone and its drop targets.
    pub(super) fn setup_pin_zone(widgets: &FluxAppWidgets, sender: &AsyncComponentSender<Self>) {
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
    }

    pub(super) fn setup_sidebar_context_menu(
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        // Right-click on empty sidebar space → offer "New Section".
        // Uses Bubble phase so it only fires when no child row claimed the event.
        {
            let new_section_rc = gtk::GestureClick::new();
            new_section_rc.set_button(3);
            new_section_rc.set_propagation_phase(gtk::PropagationPhase::Bubble);
            let s_ns = sender.clone();
            new_section_rc.connect_pressed(move |gesture, _, x, y| {
                // Only fire when no child already claimed the event (i.e. empty space).
                if gesture.current_event_state().is_empty() {
                    let menu = gtk::PopoverMenu::builder().has_arrow(false).build();
                    let menu_model = gio::Menu::new();
                    menu_model.append(Some(&tr("New Section")), Some("sidebar_empty.new_section"));
                    menu.set_menu_model(Some(&menu_model));

                    if let Some(widget) = gesture.widget() {
                        menu.set_parent(&widget);
                        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                        menu.set_pointing_to(Some(&rect));

                        let ag = gio::SimpleActionGroup::new();
                        let new_action = gio::SimpleAction::new("new_section", None);
                        let s2 = s_ns.clone();
                        new_action.connect_activate(move |_, _| {
                            s2.input(AppMsg::PromptNewSidebarSection);
                        });
                        ag.add_action(&new_action);
                        widget.insert_action_group("sidebar_empty", Some(&ag));
                        menu.popup();
                    }
                }
            });
            widgets.sidebar_box.add_controller(new_section_rc);
        }
    }

    /// Stores widget handles on the model and tracks the terminal pane height.
    pub(super) fn store_widget_refs(
        model: &mut FluxApp,
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        model.context_menu_popover.set_parent(&widgets.grid_overlay);
        model.sidebar_widget = Some(widgets.sidebar_box.clone().upcast());
        model.terminal_paned = Some(widgets.main_paned.clone());
        model.search_panel_revealer = Some(widgets.search_panel_revealer.clone());
        model.tag_panel_revealer = Some(widgets.tag_panel_revealer.clone());
        model.diff_panel_revealer = Some(widgets.diff_panel_revealer.clone());
        model.location_panel_revealer = Some(widgets.location_panel_revealer.clone());
        model.connect_panel_revealer = Some(widgets.connect_panel_revealer.clone());

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
    }

    /// Installs keyboard/mouse controllers, including back/forward side buttons.
    pub(super) fn setup_input_controllers(
        model: &FluxApp,
        widgets: &FluxAppWidgets,
        root: &adw::Window,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::ui::inputs::setup_controllers(
            root,
            &model.files.view,
            sender.clone(),
            &widgets.header_stack,
            model.config.ui.single_click,
            &model.keymap,
            &model.terminal.drawing_area,
        );

        // ── Mouse Navigation (Side Buttons 8 & 9) ──
        {
            let mouse_nav = gtk::GestureClick::new();
            mouse_nav.set_button(0);

            let s_back = sender.clone();
            let s_forward = sender.clone();
            let back_btn = model
                .config
                .shortcuts
                .back
                .as_deref()
                .and_then(crate::utils::helpers::parse_mouse_button)
                .unwrap_or(8);
            let forward_btn = model
                .config
                .shortcuts
                .forward
                .as_deref()
                .and_then(crate::utils::helpers::parse_mouse_button)
                .unwrap_or(9);

            mouse_nav.connect_pressed(move |gesture, _, _, _| {
                let btn = gesture.current_button();
                if btn == back_btn {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    s_back.input(AppMsg::GoBack);
                } else if btn == forward_btn {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    s_forward.input(AppMsg::GoForward);
                }
            });

            root.add_controller(mouse_nav);
        }
    }

    /// Applies window size, maximize state, resources and the terminal widget.
    pub(super) fn setup_window(model: &FluxApp, widgets: &FluxAppWidgets, root: &adw::Window) {
        root.set_default_size(
            model.config.ui.startup_window_width,
            model.config.ui.startup_window_height,
        );

        if model.config.ui.start_maximized {
            root.maximize();
        }

        root.connect_realize(|_| {
            crate::utils::helpers::register_resources();
        });

        let terminal_widget = model.terminal.drawing_area.clone();
        let terminal_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        terminal_box.append(&terminal_widget);
        widgets.terminal_revealer.set_child(Some(&terminal_box));
    }

    pub(super) fn schedule_startup_tasks(
        model: &FluxApp,
        sender: &AsyncComponentSender<Self>,
        open_archive: Option<PathBuf>,
    ) {
        if let Some(archive_path) = open_archive {
            let s = sender.clone();
            gtk::glib::idle_add_local_once(move || {
                s.input(AppMsg::EnterArchive(archive_path));
            });
        }

        if model.config.ui.enable_file_indexing {
            gtk::glib::idle_add_local_once(|| {
                crate::services::search::indexer::build_home_index_async();
            });
        }
    }
}
