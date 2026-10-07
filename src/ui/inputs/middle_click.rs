use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::ui::inputs::util::find_flux_path_ancestor;
use adw::gdk;
use gtk::prelude::*;
use relm4::prelude::*;

/// Middle-click on a file/dir item: Ctrl opens a new instance, otherwise opens a new tab.
pub fn setup_middle_click(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);

    gesture.connect_pressed(move |gesture, _, x, y| {
        if gesture.current_button() != constants::MOUSE_MIDDLE {
            return;
        }
        let Some(widget) = gesture.widget() else {
            return;
        };
        let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) else {
            return;
        };
        let Some(path) = find_flux_path_ancestor(picked) else {
            return;
        };

        let modifiers = gesture.current_event_state();
        if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            if crate::utils::helpers::open_new_instance(&path) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        } else {
            sender.input(AppMsg::NewTab(Some(path)));
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });

    window.add_controller(gesture);
}
