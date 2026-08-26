use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::ui::keymap::KeyMap;
use adw::gdk;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// Attaches all input controllers to the window/view.
///
/// `terminal_area` is the [`gtk::DrawingArea`] that backs the embedded terminal.
/// When it holds keyboard focus, `Tab` is yielded to it (shell completion) instead
/// of being consumed by the file-grid cycle shortcut.
pub fn setup_controllers(
    window: &impl IsA<gtk::Widget>,
    grid_view: &gtk::GridView,
    sender: relm4::AsyncComponentSender<FluxApp>,
    header_stack: &gtk::Stack,
    config_single_click: bool,
    keymap: &KeyMap,
    terminal_area: &gtk::DrawingArea,
) {
    grid_view.set_enable_rubberband(false);

    // 1. Primary Navigation
    let nav_controller = gtk::EventControllerKey::new();
    let sender_nav = sender.clone();
    let header_stack_nav = header_stack.clone();

    nav_controller.connect_key_pressed(move |ctrl, keyval, _, state| {
        let page_name = header_stack_nav.visible_child_name().unwrap_or_default();
        FluxApp::handle_key_event(ctrl, keyval, state, &sender_nav, page_name.as_str())
    });
    window.add_controller(nav_controller);

    // 2. Single-Click Toggle (Control/Shift)
    let click_toggle = gtk::EventControllerKey::new();
    let grid_view_t1 = grid_view.clone();

    click_toggle.connect_key_pressed(move |_, keyval, _, _| {
        let is_mod = keyval == gdk::Key::Control_L
            || keyval == gdk::Key::Control_R
            || keyval == gdk::Key::Shift_L
            || keyval == gdk::Key::Shift_R;

        if is_mod {
            grid_view_t1.set_enable_rubberband(true);
            if config_single_click {
                grid_view_t1.set_single_click_activate(false);
            }
        }
        glib::Propagation::Proceed
    });

    let grid_view_t2 = grid_view.clone();
    click_toggle.connect_key_released(move |_, keyval, _, _| {
        let is_mod = keyval == gdk::Key::Control_L
            || keyval == gdk::Key::Control_R
            || keyval == gdk::Key::Shift_L
            || keyval == gdk::Key::Shift_R;

        if is_mod {
            grid_view_t2.set_enable_rubberband(false);
            if config_single_click {
                grid_view_t2.set_single_click_activate(true);
            }
        }
    });
    window.add_controller(click_toggle);

    // 3. Middle Click
    let middle_click = gtk::GestureClick::new();
    middle_click.set_button(0);

    middle_click.connect_pressed(move |gesture, _, x, y| {
        let button = gesture.current_button();
        if button == constants::MOUSE_MIDDLE {
            if let Some(widget) = gesture.widget() {
                if let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) {
                    let mut current: Option<gtk::Widget> = Some(picked);
                    while let Some(w) = current {
                        let name = w.widget_name().to_string();
                        if name.starts_with('/')
                            || name.starts_with("trash://")
                            || name.starts_with("smb://")
                            || name.starts_with("sftp://")
                            || name.starts_with("ftp://")
                            || name.starts_with("nfs://")
                            || name.starts_with("archive://")
                        {
                            let path = std::path::PathBuf::from(&name);
                            if crate::utils::helpers::open_new_instance(&path) {
                                gesture.set_state(gtk::EventSequenceState::Claimed);
                            }
                            break;
                        }
                        current = w.parent();
                    }
                }
            }
        }
    });
    window.add_controller(middle_click);

    // 4. Capture Phase
    let capture = gtk::EventControllerKey::new();
    capture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let sender_cap = sender.clone();
    let terminal_area_cap = terminal_area.clone();

    capture.connect_key_pressed(move |_ctrl, keyval, _keycode, state| {
        let is_ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let is_shift = state.contains(gdk::ModifierType::SHIFT_MASK);

        match keyval {
            gdk::Key::Insert if is_ctrl => {
                sender_cap.input(AppMsg::AddToSidebarPermanent);
                glib::Propagation::Stop
            }
            gdk::Key::Insert => {
                sender_cap.input(AppMsg::AddExclusive(None));
                glib::Propagation::Stop
            }
            gdk::Key::End if is_ctrl => {
                sender_cap.input(AppMsg::ClearExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::Page_Up if is_ctrl => {
                sender_cap.input(AppMsg::PrevExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::Page_Down if is_ctrl => {
                sender_cap.input(AppMsg::NextExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::F8 => {
                sender_cap.input(AppMsg::ToggleSidebar);
                glib::Propagation::Stop
            }
            gdk::Key::t | gdk::Key::T if is_ctrl => {
                if terminal_area_cap.has_focus() {
                    return glib::Propagation::Proceed;
                }
                if is_shift {
                    sender_cap.input(AppMsg::OpenTagNavigator);
                } else {
                    sender_cap.input(AppMsg::OpenTagPicker);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Tab => {
                // When the terminal DrawingArea holds focus, let Tab pass through
                // to its own EventControllerKey so the shell sees \t for completion.
                if terminal_area_cap.has_focus() {
                    return glib::Propagation::Proceed;
                }
                sender_cap.input(AppMsg::NextExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::z if is_ctrl => {
                // Let the terminal shell handle Ctrl+Z (job control) if it has focus.
                if terminal_area_cap.has_focus() {
                    return glib::Propagation::Proceed;
                }
                let is_shift = state.contains(gdk::ModifierType::SHIFT_MASK);
                if is_shift {
                    sender_cap.input(AppMsg::Redo);
                } else {
                    sender_cap.input(AppMsg::Undo);
                }
                glib::Propagation::Stop
            }
            gdk::Key::y if is_ctrl => {
                if terminal_area_cap.has_focus() {
                    return glib::Propagation::Proceed;
                }
                sender_cap.input(AppMsg::Redo);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(capture);

    // 5. History Navigation
    let history = gtk::ShortcutController::new();

    let sender_hist_back = sender.clone();
    let shortcut_back = gtk::Shortcut::new(
        Some(keymap.back.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            sender_hist_back.input(AppMsg::GoBack);
            glib::Propagation::Stop
        })),
    );

    let sender_hist_forward = sender.clone();
    let shortcut_forward = gtk::Shortcut::new(
        Some(keymap.forward.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            sender_hist_forward.input(AppMsg::GoForward);
            glib::Propagation::Stop
        })),
    );

    history.add_shortcut(shortcut_back);
    history.add_shortcut(shortcut_forward);
    window.add_controller(history);

    // 6. Global KeyMap Bindings
    let global_shortcuts = gtk::ShortcutController::new();
    global_shortcuts.set_scope(gtk::ShortcutScope::Global);

    let s_refresh = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.refresh.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_refresh.input(AppMsg::Refresh);
            glib::Propagation::Stop
        })),
    ));

    let s_sidebar = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(gtk::ShortcutTrigger::parse_string("F8").unwrap()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_sidebar.input(AppMsg::ToggleSidebar);
            glib::Propagation::Stop
        })),
    ));

    let s_terminal = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(gtk::ShortcutTrigger::parse_string("F4").unwrap()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_terminal.input(AppMsg::ToggleTerminal);
            glib::Propagation::Stop
        })),
    ));

    let s_search = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.search.clone()),
        Some(gtk::CallbackAction::new(move |widget, _| {
            // Don't try focus if a rename entry is currently active
            let rename_is_focused = widget
                .root()
                .and_then(|root| root.focus())
                .map(|focused: gtk::Widget| {
                    focused
                        .css_classes()
                        .iter()
                        .any(|c: &gtk::glib::GString| c.as_str() == "flux-rename-entry")
                })
                .unwrap_or(false);

            if rename_is_focused {
                return glib::Propagation::Proceed;
            }

            s_search.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
            glib::Propagation::Stop
        })),
    ));

    let s_hidden = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.toggle_hidden.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_hidden.input(AppMsg::ToggleHidden);
            glib::Propagation::Stop
        })),
    ));

    let s_delete = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.delete.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_delete.input(AppMsg::Delete);
            glib::Propagation::Stop
        })),
    ));

    let s_undo = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(gtk::ShortcutTrigger::parse_string("<Primary>z").unwrap()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_undo.input(AppMsg::Undo);
            glib::Propagation::Stop
        })),
    ));

    let s_redo1 = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(gtk::ShortcutTrigger::parse_string("<Primary><Shift>z").unwrap()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_redo1.input(AppMsg::Redo);
            glib::Propagation::Stop
        })),
    ));

    let s_redo2 = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(gtk::ShortcutTrigger::parse_string("<Primary>y").unwrap()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_redo2.input(AppMsg::Redo);
            glib::Propagation::Stop
        })),
    ));

    let s_prop = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.properties.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_prop.input(AppMsg::TriggerRenameSelection);
            glib::Propagation::Stop
        })),
    ));

    let s_open = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.open.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_open.input(AppMsg::Open(None));
            glib::Propagation::Stop
        })),
    ));

    let s_root = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.root.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_root.input(AppMsg::Navigate(PathBuf::from("/")));
            glib::Propagation::Stop
        })),
    ));

    let s_tag = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(gtk::ShortcutTrigger::parse_string("<Primary>t").unwrap()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_tag.input(AppMsg::OpenTagPicker);
            glib::Propagation::Stop
        })),
    ));

    let s_tag_nav = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(gtk::ShortcutTrigger::parse_string("<Primary><Shift>t").unwrap()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_tag_nav.input(AppMsg::OpenTagNavigator);
            glib::Propagation::Stop
        })),
    ));

    // 6.1 Icon Management Shortcuts
    let s_icon_picker = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.change_icon.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_icon_picker.input(AppMsg::TriggerIconPicker);
            glib::Propagation::Stop
        })),
    ));

    let s_icon_reset = sender.clone();
    global_shortcuts.add_shortcut(gtk::Shortcut::new(
        Some(keymap.reset_icon.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            s_icon_reset.input(AppMsg::TriggerResetIcon);
            glib::Propagation::Stop
        })),
    ));

    window.add_controller(global_shortcuts);

    // 7. Swipe Gestures
    let swipe = gtk::GestureSwipe::new();
    let sender_swipe = sender.clone();

    swipe.connect_swipe(move |_, velocity_x, _| {
        if velocity_x > constants::SWIPE_VELOCITY_THRESHOLD {
            sender_swipe.input(AppMsg::GoBack);
        } else if velocity_x < -constants::SWIPE_VELOCITY_THRESHOLD {
            sender_swipe.input(AppMsg::GoForward);
        }
    });
    window.add_controller(swipe);

    // 8. Mouse Back/Forward
    let mouse_nav = gtk::GestureClick::new();
    mouse_nav.set_button(0);

    mouse_nav.connect_pressed(|gesture, _, _, _| {
        let button = gesture.current_button();
        if button == constants::MOUSE_BACK || button == constants::MOUSE_FORWARD {
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });

    let sender_mouse = sender.clone();
    mouse_nav.connect_released(move |gesture, _, _, _| {
        let button = gesture.current_button();
        let state = gesture.current_event_state();
        let modifiers = state & gtk::accelerator_get_default_mod_mask();

        if button == constants::MOUSE_BACK && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            sender_mouse.input(AppMsg::JumpToRecent(0));
        }
    });
    window.add_controller(mouse_nav);

    // 9. Settings window
    let parent_window = window.root().and_downcast::<gtk::Window>();
    let settings_shortcut = gtk::ShortcutController::new();
    settings_shortcut.add_shortcut(gtk::Shortcut::new(
        Some(keymap.settings.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            let controller = crate::ui::SettingsWindow::builder().launch(());
            if let Some(parent) = &parent_window {
                controller.widget().set_transient_for(Some(parent));
            }
            controller.widget().present();
            controller.detach();
            gtk::glib::Propagation::Stop
        })),
    ));
    window.add_controller(settings_shortcut);

    // 10. Click-on-empty-space deselection
    setup_deselect_on_background_click(grid_view, sender.clone());

    // 11. Ctrl+Right-click → secondary MIME-matched context menu
    crate::ui::secondary_context_menu::setup_secondary_menu_gesture(window, sender);
}

/// Attaches a primary-button gesture to the `GridView` that clears the
/// `MultiSelection` when the user clicks on empty background space, unless
/// Ctrl or Shift is held (multi-selection modifiers).
fn setup_deselect_on_background_click(
    grid_view: &gtk::GridView,
    _sender: relm4::AsyncComponentSender<FluxApp>,
) {
    let deselect = gtk::GestureClick::new();
    deselect.set_button(1);
    // Bubble phase so individual GridView child cells get first refusal.
    deselect.set_propagation_phase(gtk::PropagationPhase::Bubble);

    let grid_view_weak = grid_view.downgrade();

    deselect.connect_pressed(move |gesture, _, x, y| {
        let modifiers = gesture
            .current_event()
            .map(|e| e.modifier_state())
            .unwrap_or(gdk::ModifierType::empty());

        let multi_select =
            modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);

        if multi_select {
            return;
        }

        let Some(grid_view) = grid_view_weak.upgrade() else {
            return;
        };

        // Walk upward from the picked widget, if any ancestor carries a
        // filesystem path name the click landed on a file/dir item.
        let hit_item = grid_view
            .pick(x, y, gtk::PickFlags::DEFAULT)
            .map(|picked| {
                let mut current: Option<gtk::Widget> = Some(picked);
                loop {
                    match current {
                        None => break false,
                        Some(ref w) => {
                            let name = w.widget_name().to_string();
                            if name.starts_with('/')
                                || name.starts_with("trash://")
                                || name.starts_with("smb://")
                                || name.starts_with("sftp://")
                                || name.starts_with("ftp://")
                                || name.starts_with("nfs://")
                                || name.starts_with("archive://")
                            {
                                break true;
                            }
                            current = w.parent();
                        }
                    }
                }
            })
            .unwrap_or(false);

        if !hit_item {
            if let Some(model) = grid_view.model().and_downcast::<gtk::MultiSelection>() {
                model.unselect_all();
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });

    grid_view.add_controller(deselect);
}
