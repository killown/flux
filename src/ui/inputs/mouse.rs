use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use adw::gdk;
use gtk::prelude::*;
use relm4::prelude::*;

/// Two-finger horizontal swipe → back/forward.
pub fn setup_swipe(window: &impl IsA<gtk::Widget>, sender: relm4::AsyncComponentSender<FluxApp>) {
    let swipe = gtk::GestureSwipe::new();
    swipe.connect_swipe(move |_, velocity_x, _| {
        if velocity_x > constants::SWIPE_VELOCITY_THRESHOLD {
            sender.input(AppMsg::GoBack);
        } else if velocity_x < -constants::SWIPE_VELOCITY_THRESHOLD {
            sender.input(AppMsg::GoForward);
        }
    });
    window.add_controller(swipe);
}

/// Mouse back/forward buttons. Ctrl+Back jumps to the most recent location.
pub fn setup_mouse_back_forward(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);

    gesture.connect_pressed(|gesture, _, _, _| {
        let button = gesture.current_button();
        if button == constants::MOUSE_BACK || button == constants::MOUSE_FORWARD {
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });

    gesture.connect_released(move |gesture, _, _, _| {
        let button = gesture.current_button();
        let state = gesture.current_event_state();
        let modifiers = state & gtk::accelerator_get_default_mod_mask();

        if button == constants::MOUSE_BACK && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            sender.input(AppMsg::JumpToRecent(0));
        }
    });

    window.add_controller(gesture);
}
