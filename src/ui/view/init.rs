use super::component::FluxAppWidgets;
use crate::domain::model::{AppMsg, FluxApp};
use crate::i18n::tr;
use adw::gdk;
use adw::prelude::*;
use gtk::gio;
use gtk::glib::{self};
use relm4::prelude::*;
use std::path::PathBuf;

/// A named git menu action paired with a constructor for the message it sends.
type GitActionEntry = (&'static str, fn() -> AppMsg);

impl FluxApp {
    /// Attaches the tab bar, header widget, main menu and sidebar wrapper.
    pub(super) fn attach_tabs_and_header(
        model: &mut FluxApp,
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("attach_tabs_and_header");
        widgets.toolbar_view.add_top_bar(&model.tab_bar);
        widgets.tab_view_container.append(&model.tab_view);
        if let Some(first_tab) = model.tabs.first() {
            Self::connect_tab_scroller(&first_tab.scroller, sender);
        }

        model.header_widget = Some(widgets.header_bar.clone().upcast());

        let main_menu = Self::build_main_menu();
        widgets.main_menu_popover.set_menu_model(Some(&main_menu));

        // Git actions dropdown (only shown when is_in_git_repo).
        //
        // Layout: status, remote and log at the top, then the staged/unstaged
        // filters, branch switching with diff copy, and the discard pair.
        // Heavier workflows (commit, stash, push/pull, patches) belong in the
        // embedded terminal.
        {
            let menu = gtk::gio::Menu::new();

            let primary = gtk::gio::Menu::new();
            primary.append(Some(&tr("Git Status")), Some("git.status"));
            primary.append(Some(&tr("Open Remote in Browser")), Some("git.open-remote"));
            primary.append(Some(&tr("Show Log…")), Some("git.log"));
            menu.append_section(None, &primary);

            let filters = gtk::gio::Menu::new();
            filters.append(Some(&tr("Staged Only")), Some("git.status-staged"));
            filters.append(Some(&tr("Unstaged Only")), Some("git.status-unstaged"));
            menu.append_section(None, &filters);

            let branch = gtk::gio::Menu::new();
            branch.append(Some(&tr("Switch Branch…")), Some("git.switch-branch"));
            branch.append(Some(&tr("Copy Diff to Clipboard")), Some("git.copy-diff"));
            menu.append_section(None, &branch);

            let discard = gtk::gio::Menu::new();
            discard.append(
                Some(&tr("Discard Selected Changes")),
                Some("git.discard-selected"),
            );
            discard.append(
                Some(&tr("Restore Last Discard")),
                Some("git.restore-discard"),
            );
            menu.append_section(None, &discard);

            widgets.git_menu_popover.set_menu_model(Some(&menu));
        }

        model.header_path_entry = widgets.header_path_entry.downgrade();

        let git_actions = gtk::gio::SimpleActionGroup::new();

        {
            let s = sender.clone();
            let a = gtk::gio::SimpleAction::new("status", None);
            a.connect_activate(move |_, _| {
                let _ = s.input_sender().send(AppMsg::ShowGitStatusView);
            });
            git_actions.add_action(&a);
        }

        let simple_git_actions: [GitActionEntry; 8] = [
            ("status-staged", || AppMsg::ShowGitStatusStaged),
            ("status-unstaged", || AppMsg::ShowGitStatusUnstaged),
            ("log", || AppMsg::GitShowLog),
            ("copy-diff", || AppMsg::GitCopyDiff),
            ("switch-branch", || AppMsg::PromptGitSwitchBranch),
            ("open-remote", || AppMsg::GitOpenRemote),
            ("discard-selected", || AppMsg::GitDiscardSelected),
            ("restore-discard", || AppMsg::GitRestoreLastDiscard),
        ];
        for (name, make_msg) in simple_git_actions {
            let s = sender.clone();
            let a = gtk::gio::SimpleAction::new(name, None);
            a.connect_activate(move |_, _| {
                let _ = s.input_sender().send(make_msg());
            });
            git_actions.add_action(&a);
        }

        widgets
            .git_menu_popover
            .insert_action_group("git", Some(&git_actions));

        let sidebar_wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        if model.sidebar.widget().parent().is_none() {
            sidebar_wrapper.append(model.sidebar.widget());
        }
        sidebar_wrapper.append(&model.network_section);
        widgets.sidebar_container.set_child(Some(&sidebar_wrapper));
    }

    /// Smallest sidebar width that still fits its rows, from their CSS-aware minimum.
    fn sidebar_min_width(scroller: &gtk::ScrolledWindow) -> i32 {
        let (min, _, _, _) = scroller
            .child()
            .and_then(|c| c.downcast::<gtk::Viewport>().ok())
            .and_then(|v| v.child())
            .map(|w| w.measure(gtk::Orientation::Horizontal, -1))
            .unwrap_or((0, 0, 0, 0));
        min.max(160)
    }

    /// Wires the sidebar resize handle (hover cursor and drag gesture).
    pub(super) fn setup_sidebar_resize(
        model: &FluxApp,
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("setup_sidebar_resize");
        {
            let sidebar_box_weak = widgets.sidebar_box.downgrade();
            let scroller_weak = widgets.sidebar_container.downgrade();
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
                                let (rx, _) = crate::utils::helpers::translate_point(
                                    &handle, &root_win, x, 0.0,
                                )
                                .unwrap_or((x, 0.0));
                                start_rx.set(rx);
                            }
                        }
                    }
                });
            }

            {
                let s_box_w = sidebar_box_weak.clone();
                let sc_w = scroller_weak.clone();
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
                                        crate::utils::helpers::translate_point(
                                            &handle, &root_win, curr_x, 0.0,
                                        )
                                    {
                                        let delta_x = curr_root_x - start_rx.get();
                                        let min_w = sc_w
                                            .upgrade()
                                            .map(|sc| Self::sidebar_min_width(&sc))
                                            .unwrap_or(160);
                                        let new_w = (start_w.get() + delta_x as i32)
                                            .clamp(min_w, 500.max(min_w));

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
                let sc_w = scroller_weak;
                let s_sender = sender.clone();
                let ready_c = is_ready;

                drag_gesture.connect_drag_end(move |gesture, _, _| {
                    if !ready_c.get() {
                        return;
                    }
                    if let Some(s) = s_box_w.upgrade() {
                        let min_w = sc_w
                            .upgrade()
                            .map(|sc| Self::sidebar_min_width(&sc))
                            .unwrap_or(160);
                        let final_width = s.width().clamp(min_w, 500.max(min_w));
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
        crate::hit!("setup_pin_zone");
        let pin_zone = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::Start)
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

        // Overlay on top of the sidebar so showing it never shifts the list.
        // The drop target lives on the Overlay (not sidebar_box) so moving the
        // pointer onto the zone doesn't count as leaving the sidebar.
        let overlay = widgets
            .sidebar_box
            .parent()
            .and_downcast::<gtk::Overlay>()
            .expect("sidebar_box is the child of an Overlay");
        overlay.add_overlay(&pin_zone);

        type Slot = std::rc::Rc<std::cell::Cell<Option<glib::SourceId>>>;
        fn cancel(slot: &Slot) {
            if let Some(id) = slot.take() {
                id.remove();
            }
        }
        fn is_pending(slot: &Slot) -> bool {
            let cur = slot.take();
            let pending = cur.is_some();
            slot.set(cur);
            pending
        }
        let show_timer: Slot = Slot::default();
        let hide_timer: Slot = Slot::default();

        {
            let sidebar_ft = gtk::DropTarget::builder()
                .actions(gdk::DragAction::COPY | gdk::DragAction::MOVE)
                .preload(false)
                .build();
            sidebar_ft.set_types(&[gdk::FileList::static_type()]);

            let (pz, st, ht) = (pin_zone.clone(), show_timer.clone(), hide_timer.clone());
            sidebar_ft.connect_enter(move |_, _, _| {
                cancel(&ht);
                if !pz.is_visible() && !is_pending(&st) {
                    let (pz2, st2) = (pz.clone(), st.clone());
                    let id = glib::timeout_add_local_once(
                        std::time::Duration::from_millis(
                            crate::ui::constants::SIDEBAR_PIN_ZONE_DELAY_MS,
                        ),
                        move || {
                            st2.set(None);
                            pz2.set_visible(true);
                        },
                    );
                    st.set(Some(id));
                }
                gdk::DragAction::empty()
            });

            let (pz, st, ht) = (pin_zone.clone(), show_timer.clone(), hide_timer.clone());
            sidebar_ft.connect_leave(move |_| {
                cancel(&st);
                cancel(&ht);
                let (pz2, ht2) = (pz.clone(), ht.clone());
                let id =
                    glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
                        ht2.set(None);
                        pz2.set_visible(false);
                    });
                ht.set(Some(id));
            });
            overlay.add_controller(sidebar_ft);
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
            let (st_drop, ht_drop) = (show_timer.clone(), hide_timer.clone());
            pin_zone_dt.connect_drop(move |_, value, _, _| {
                cancel(&st_drop);
                cancel(&ht_drop);
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

    /// Keeps keyboard focus out of the header bar.
    ///
    /// When the focused widget is hidden, destroyed or made insensitive (view
    /// mode switch, panel close, history running out, ...) GTK falls back to
    /// the first focusable widget, which landed on the back button, a
    /// breadcrumb, etc. and painted a stray focus ring. Everything in the
    /// header is made non-focusable except text entries (path / search), and
    /// popover contents are left alone. Mouse use is unaffected.
    pub(super) fn lock_header_focus(root: &gtk::Widget) {
        let mut child = root.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            if w.is::<gtk::Popover>() || w.is::<gtk::Editable>() {
                continue;
            }
            w.set_focusable(false);
            Self::lock_header_focus(&w);
        }
    }

    pub(super) fn setup_sidebar_context_menu(
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("setup_sidebar_context_menu");
        // Right-click on empty sidebar space → "New Section" and panel shortcuts.
        // On the ScrolledWindow, not sidebar_box: the empty area below the last
        // row is inside it, and GtkScrolledWindow eats the click before it can
        // bubble out. Bubble phase lets rows with their own menu win first,
        // pick() catches non-claiming children like section labels.
        let new_section_rc = gtk::GestureClick::new();
        new_section_rc.set_button(3);
        new_section_rc.set_propagation_phase(gtk::PropagationPhase::Bubble);

        let s_ns = sender.clone();
        new_section_rc.connect_pressed(move |gesture, _, x, y| {
            let ui_cfg = crate::utils::load_config().ui;
            if (ui_cfg.sidebar_menu_requires_ctrl || ui_cfg.panel_menu_requires_ctrl)
                && !gesture
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return;
            }
            if let Some(widget) = gesture.widget() {
                if widget
                    .pick(x, y, gtk::PickFlags::DEFAULT)
                    .is_some_and(|picked| picked.is::<gtk::ListBoxRow>())
                {
                    return;
                }
            }

            let menu = gtk::PopoverMenu::builder().has_arrow(false).build();
            let menu_model = gio::Menu::new();

            menu_model.append(Some(&tr("New Section")), Some("sidebar_empty.new_section"));
            menu_model.append(
                Some(&tr("Add Actions")),
                Some("sidebar_empty.add_panel_actions"),
            );
            menu_model.append(
                Some(&tr("Add Search")),
                Some("sidebar_empty.add_panel_search"),
            );
            menu_model.append(Some(&tr("Add Tags")), Some("sidebar_empty.add_panel_tags"));

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

                for (name, panel_name, path, icon) in [
                    (
                        "add_panel_actions",
                        "Actions",
                        "actions://",
                        "application-x-executable-symbolic",
                    ),
                    (
                        "add_panel_search",
                        "Search",
                        "search://",
                        "system-search-symbolic",
                    ),
                    ("add_panel_tags", "Tags", "tags://", "tag-symbolic"),
                ] {
                    let act = gio::SimpleAction::new(name, None);
                    let s3 = s_ns.clone();
                    let name = panel_name.to_string();
                    let path = path.to_string();
                    let icon = icon.to_string();
                    act.connect_activate(move |_, _| {
                        s3.input(AppMsg::AddPanelSidebarEntry {
                            name: name.clone(),
                            path: path.clone(),
                            icon: icon.clone(),
                        });
                    });
                    ag.add_action(&act);
                }

                widget.insert_action_group("sidebar_empty", Some(&ag));
                menu.popup();
            }
        });

        widgets.sidebar_container.add_controller(new_section_rc);
    }

    /// Stores widget handles on the model and tracks the terminal pane height.
    pub(super) fn store_widget_refs(
        model: &mut FluxApp,
        widgets: &FluxAppWidgets,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("store_widget_refs");
        model.context_menu_popover.set_parent(&widgets.grid_overlay);
        model.sidebar_widget = Some(widgets.sidebar_box.clone().upcast());
        model.terminal_paned = Some(widgets.main_paned.clone());
        model.search_panel_revealer = Some(widgets.search_panel_revealer.clone());
        model.tag_panel_revealer = Some(widgets.tag_panel_revealer.clone());
        model.diff_panel_revealer = Some(widgets.diff_panel_revealer.clone());
        model.preview_panel_revealer = Some(widgets.preview_panel_revealer.clone());
        model.location_panel_revealer = Some(widgets.location_panel_revealer.clone());
        model.connect_panel_revealer = Some(widgets.connect_panel_revealer.clone());
        model.actions_panel_revealer = Some(widgets.actions_panel_revealer.clone());
        model.quick_list_panel_revealer = Some(widgets.quick_list_panel_revealer.clone());
        model.terminal_panel_revealer = Some(widgets.terminal_panel_revealer.clone());
        model.properties_panel_revealer = Some(widgets.properties_panel_revealer.clone());
        model.main_stack = Some(widgets.main_stack.clone());
        model.fullscreen_terminal_box = Some(widgets.fullscreen_terminal_box.clone());

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
        crate::hit!("setup_input_controllers");
        crate::ui::inputs::setup_controllers(
            root,
            &model.tab_view,
            sender.clone(),
            &widgets.header_stack,
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
        crate::hit!("setup_window");
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
        terminal_widget.add_css_class("flux-terminal-area");
        let terminal_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        terminal_box.append(&terminal_widget);
        widgets.terminal_revealer.set_child(Some(&terminal_box));

        // Sidebar terminal cleanup: the bottom terminal's shutdown is wired
        // in `build_terminal`, this is the equivalent for the panel.
        let sidebar_terminal = model.sidebar_terminal.clone();
        root.connect_close_request(move |_| {
            if let Some(ref term) = sidebar_terminal {
                term.kill_shell();
            }
            glib::Propagation::Proceed
        });
    }

    pub(super) fn schedule_startup_tasks(
        model: &FluxApp,
        sender: &AsyncComponentSender<Self>,
        open_archive: Option<PathBuf>,
    ) {
        crate::hit!("schedule_startup_tasks");
        if let Some(archive_path) = open_archive {
            let s = sender.clone();
            gtk::glib::idle_add_local_once(move || {
                s.input(AppMsg::EnterArchive(archive_path));
            });
        }

        if model.config.ui.enable_file_indexing {
            gtk::glib::idle_add_local_once(|| {
                crate::infra::search::indexer::build_home_index_async();
            });
        }
    }
}
