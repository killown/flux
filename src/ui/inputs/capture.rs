use crate::model::{AppMsg, FluxApp};
use adw::gdk;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// Capture-phase key controller. Handles shortcuts that must win over
/// focused children (Tab cycling, Ctrl+Z/Y, Insert/End/PageUp/PageDown,
/// F-keys, Ctrl+Shift+T tags). Yields to editable widgets and the terminal.
pub fn setup_capture(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
    terminal_area: &gtk::DrawingArea,
) {
    let capture = gtk::EventControllerKey::new();
    capture.set_propagation_phase(gtk::PropagationPhase::Capture);

    let terminal_area = terminal_area.clone();

    capture.connect_key_pressed(move |ctrl, keyval, _keycode, state| {
        let is_ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let is_shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let is_alt = state.contains(gdk::ModifierType::ALT_MASK);

        // Check if an editable text input is currently focused
        let is_editable = ctrl
            .widget()
            .and_then(|w| w.root())
            .and_then(|r| r.focus())
            .map(|f| f.type_().is_a(gtk::Editable::static_type()))
            .unwrap_or(false);

        let terminal_focused = terminal_area.has_focus();

        // Non-modifier / navigation shortcuts (ignored when typing or when the terminal has focus)
        if !is_editable && !terminal_focused && is_shift && !is_ctrl && !is_alt {
            match keyval {
                gdk::Key::L => {
                    sender.input(AppMsg::NextTab);
                    return glib::Propagation::Stop;
                }
                gdk::Key::H => {
                    sender.input(AppMsg::PrevTab);
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
        }

        match keyval {
            gdk::Key::Insert if is_ctrl => {
                sender.input(AppMsg::AddToSidebarPermanent);
                glib::Propagation::Stop
            }
            gdk::Key::Insert => {
                sender.input(AppMsg::AddExclusive(None));
                glib::Propagation::Stop
            }
            gdk::Key::End if is_ctrl => {
                sender.input(AppMsg::ClearExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::Page_Up if is_ctrl => {
                sender.input(AppMsg::PrevExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::Page_Down if is_ctrl => {
                sender.input(AppMsg::NextExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::Home if is_ctrl => {
                if terminal_focused || is_editable {
                    return glib::Propagation::Proceed;
                }
                let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
                sender.input(AppMsg::Navigate(home_dir));
                glib::Propagation::Stop
            }
            gdk::Key::F6 => {
                sender.input(AppMsg::ToggleHeaderBar);
                glib::Propagation::Stop
            }
            gdk::Key::F3 => {
                sender.input(AppMsg::ToggleCurrentFoldersFirst);
                glib::Propagation::Stop
            }
            gdk::Key::F7 if !is_ctrl && !is_shift => {
                sender.input(AppMsg::ToggleStatusBar);
                glib::Propagation::Stop
            }
            gdk::Key::F8 => {
                sender.input(AppMsg::ToggleSidebar);
                glib::Propagation::Stop
            }
            // Tags: Ctrl + Shift + T
            gdk::Key::t | gdk::Key::T if is_ctrl && is_shift && !is_alt => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::ToggleTagPanel);
                glib::Propagation::Stop
            }
            gdk::Key::Tab if is_ctrl => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                if is_shift {
                    sender.input(AppMsg::PrevTab);
                } else {
                    sender.input(AppMsg::NextTab);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Tab => {
                // When the terminal DrawingArea holds focus, let Tab pass through
                // to its own EventControllerKey so the shell sees \t for completion.
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::NextExclusive);
                glib::Propagation::Stop
            }
            gdk::Key::z if is_ctrl => {
                // Let the terminal shell handle Ctrl+Z (job control) if it has focus.
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                if is_shift {
                    sender.input(AppMsg::Redo);
                } else {
                    sender.input(AppMsg::Undo);
                }
                glib::Propagation::Stop
            }
            gdk::Key::y if is_ctrl => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::Redo);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });

    window.add_controller(capture);
}
