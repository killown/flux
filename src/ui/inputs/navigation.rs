use crate::domain::model::{AppMsg, FluxApp};
use crate::ui::keymap::KeyMap;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;

/// Primary keyboard navigation controller (delegates into `FluxApp::handle_key_event`).
pub fn setup_navigation(
    window: &impl IsA<gtk::Widget>,
    header_stack: &gtk::Stack,
    terminal_area: &gtk::DrawingArea,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    crate::hit!("setup_navigation");
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let header_stack = header_stack.clone();
    let terminal_area = terminal_area.clone();

    controller.connect_key_pressed(move |ctrl, keyval, _, state| {
        // If a terminal (bottom pane or sidebar panel) has focus, do not
        // route the key through `handle_key_event`. Otherwise type-to-search
        // would steal characters, and the embedded shell could not be typed
        // into.
        if let Some(focus) = ctrl.widget().and_then(|w| w.root()).and_then(|r| r.focus()) {
            if focus.has_css_class("flux-terminal-area") {
                return glib::Propagation::Proceed;
            }
        }

        let page_name = header_stack.visible_child_name().unwrap_or_default();
        FluxApp::handle_key_event(
            ctrl,
            keyval,
            state,
            &sender,
            page_name.as_str(),
            terminal_area.has_focus(),
        )
    });

    window.add_controller(controller);
}

/// History back/forward shortcuts (driven by the resolved [`KeyMap`]).
///
/// Each field is an `Option`: a `None` means the user disabled the shortcut
/// in `config.toml` (`back = "disabled"`), so no `gtk::Shortcut` is installed
/// for it. A disabled chord therefore falls through to the focused widget
/// exactly as if it were never registered.
pub fn setup_history(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
    keymap: &KeyMap,
) {
    crate::hit!("setup_history");
    let history = gtk::ShortcutController::new();

    if let Some(back_trigger) = keymap.back.clone() {
        let sender_back = sender.clone();
        history.add_shortcut(gtk::Shortcut::new(
            Some(back_trigger),
            Some(gtk::CallbackAction::new(move |_, _| {
                sender_back.input(AppMsg::GoBack);
                glib::Propagation::Stop
            })),
        ));
    }

    if let Some(forward_trigger) = keymap.forward.clone() {
        let sender_forward = sender;
        history.add_shortcut(gtk::Shortcut::new(
            Some(forward_trigger),
            Some(gtk::CallbackAction::new(move |_, _| {
                sender_forward.input(AppMsg::GoForward);
                glib::Propagation::Stop
            })),
        ));
    }

    window.add_controller(history);
}
