use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use adw::gdk;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;

/// Attaches all input controllers to the window/view.
pub fn setup_controllers(
    window: &impl IsA<gtk::Widget>,
    grid_view: &gtk::GridView,
    sender: ComponentSender<FluxApp>,
    header_stack: &gtk::Stack,
    config_single_click: bool,
) {
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
        if config_single_click
            && (keyval == gdk::Key::Control_L
                || keyval == gdk::Key::Control_R
                || keyval == gdk::Key::Shift_L
                || keyval == gdk::Key::Shift_R)
        {
            grid_view_t1.set_single_click_activate(false);
        }
        glib::Propagation::Proceed
    });

    let grid_view_t2 = grid_view.clone();
    click_toggle.connect_key_released(move |_, keyval, _, _| {
        if config_single_click
            && (keyval == gdk::Key::Control_L
                || keyval == gdk::Key::Control_R
                || keyval == gdk::Key::Shift_L
                || keyval == gdk::Key::Shift_R)
        {
            grid_view_t2.set_single_click_activate(true);
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
                        if name.starts_with("/") || name.starts_with("trash://") {
                            let _ = std::process::Command::new("flux")
                                .arg(std::path::PathBuf::from(name))
                                .spawn();
                            gesture.set_state(gtk::EventSequenceState::Claimed);
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

    capture.connect_key_pressed(move |_ctrl, keyval, _keycode, state| {
        let modifiers = state & gtk::accelerator_get_default_mod_mask();
        let is_ctrl = modifiers == gdk::ModifierType::CONTROL_MASK;

        match keyval {
            gdk::Key::Insert => {
                sender_cap.input(AppMsg::AddExclusive);
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
            gdk::Key::Tab => {
                sender_cap.input(AppMsg::NextExclusive);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(capture);

    // 5. History Navigation
    let history = gtk::EventControllerKey::new();
    let sender_hist = sender.clone();

    history.connect_key_pressed(move |_, keyval, _, state| {
        let modifiers = state & gtk::accelerator_get_default_mod_mask();
        let is_ctrl = modifiers == gdk::ModifierType::CONTROL_MASK;
        match keyval {
            gdk::Key::bracketleft if is_ctrl => {
                sender_hist.input(AppMsg::CycleRecent(-1));
                glib::Propagation::Stop
            }
            gdk::Key::bracketright if is_ctrl => {
                sender_hist.input(AppMsg::CycleRecent(1));
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(history);

    // 6. Swipe Gestures
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

    // 7. Mouse Back/Forward
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
}
