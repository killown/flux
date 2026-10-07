use crate::model::{AppMsg, FluxApp};
use crate::ui::keymap::KeyMap;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;

/// Primary keyboard navigation controller (delegates into `FluxApp::handle_key_event`).
pub fn setup_navigation(
    window: &impl IsA<gtk::Widget>,
    header_stack: &gtk::Stack,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    let controller = gtk::EventControllerKey::new();
    let header_stack = header_stack.clone();

    controller.connect_key_pressed(move |ctrl, keyval, _, state| {
        let page_name = header_stack.visible_child_name().unwrap_or_default();
        FluxApp::handle_key_event(ctrl, keyval, state, &sender, page_name.as_str())
    });

    window.add_controller(controller);
}

/// History back/forward shortcuts (driven by the resolved [`KeyMap`]).
pub fn setup_history(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
    keymap: &KeyMap,
) {
    let history = gtk::ShortcutController::new();

    let sender_back = sender.clone();
    history.add_shortcut(gtk::Shortcut::new(
        Some(keymap.back.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            sender_back.input(AppMsg::GoBack);
            glib::Propagation::Stop
        })),
    ));

    let sender_forward = sender;
    history.add_shortcut(gtk::Shortcut::new(
        Some(keymap.forward.clone()),
        Some(gtk::CallbackAction::new(move |_, _| {
            sender_forward.input(AppMsg::GoForward);
            glib::Propagation::Stop
        })),
    ));

    window.add_controller(history);
}
