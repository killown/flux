use crate::model::{AppMsg, FluxApp};
use adw::gdk;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// Capture-phase key controller. Handles shortcuts that must win over
/// focused children (Tab cycling, Ctrl+Z/Y, Insert/End/PageUp/PageDown,
/// F-keys, Ctrl+Shift+T/A/L, Ctrl+Alt+<letter> cycle). Yields to editable
/// widgets and the terminal.
///
/// Rule of thumb: if a chord must work no matter which widget currently
/// has focus (including widgets inside right-side panels), it belongs
/// here, not in a window-level `ShortcutController`. Capture-phase
/// `EventControllerKey` runs before any focused child can pre-empt the
/// event, bubble-phase shortcuts do not.
pub fn setup_capture(window: &impl IsA<gtk::Widget>, sender: relm4::AsyncComponentSender<FluxApp>) {
    let capture = gtk::EventControllerKey::new();
    capture.set_propagation_phase(gtk::PropagationPhase::Capture);

    capture.connect_key_pressed(move |ctrl, keyval, _keycode, state| {
        let is_ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let is_shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let is_alt = state.contains(gdk::ModifierType::ALT_MASK);

        // Both terminals (bottom pane and right sidebar panel) carry this
        // class on their `DrawingArea`, so checking the focused widget's
        // class catches either one regardless of which pane is active.
        let focused_widget = ctrl.widget().and_then(|w| w.root()).and_then(|r| r.focus());

        let is_editable = focused_widget
            .as_ref()
            .map(|f| f.type_().is_a(gtk::Editable::static_type()))
            .unwrap_or(false);

        let terminal_focused = focused_widget
            .as_ref()
            .is_some_and(|f| f.has_css_class("flux-terminal-area"));

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

        // Ctrl+Alt+<letter> folder cycle. Consumed at capture phase so the
        // ShortcutController (Ctrl+W = close tab, etc.) never sees the chord.
        if is_ctrl && is_alt && !is_shift && !is_editable && !terminal_focused {
            if keyval == gdk::Key::backslash {
                sender.input(AppMsg::CycleByLetterCancel);
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::slash {
                sender.input(AppMsg::CycleByLetterReanchor);
                return glib::Propagation::Stop;
            }
            if let Some(c) = keyval.to_unicode() {
                let c = c.to_ascii_lowercase();
                if c.is_ascii_lowercase() {
                    sender.input(AppMsg::CycleByLetter(c));
                    return glib::Propagation::Stop;
                }
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
            gdk::Key::e | gdk::Key::E if is_ctrl && is_shift && !is_alt => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::CycleFilterMode);
                glib::Propagation::Stop
            }
            // Ctrl+Shift+A: toggle contextual-actions panel. Capture phase
            // so a focused widget inside the panel cannot pre-empt the chord.
            gdk::Key::a | gdk::Key::A if is_ctrl && is_shift && !is_alt => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::ToggleActionsPanel);
                glib::Propagation::Stop
            }
            // Ctrl+Shift+L: toggle connect-to-server panel.
            gdk::Key::l | gdk::Key::L if is_ctrl && is_shift && !is_alt => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::ToggleConnectPanel);
                glib::Propagation::Stop
            }
            // Ctrl+Shift+F: toggle the glob-pattern filter bar. Capture phase so
            // it fires even while the filter entry itself has focus, which is the
            // one place a user is most likely to press it to dismiss the bar.
            gdk::Key::f | gdk::Key::F if is_ctrl && is_shift && !is_alt => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::ToggleFilterBar);
                glib::Propagation::Stop
            }
            // Ctrl+M: toggle ephemeral MIME filter on the selection. Moved
            // here for the same reason as Ctrl+Shift+A: it must work while
            // the peek-preview panel (or any other focused panel) is open.
            gdk::Key::m | gdk::Key::M if is_ctrl && !is_shift && !is_alt => {
                if terminal_focused || is_editable {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::FilterBySelectedMime);
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
            // F4 toggles the terminal panel. When either terminal has focus,
            // the terminal's own key controller fires first and handles this
            // key, so we yield there to avoid a double fire.
            gdk::Key::F4 => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::ToggleTerminal);
                glib::Propagation::Stop
            }
            // Wide preview: F11 is left to the shell when the terminal has focus.
            gdk::Key::F11 if !is_ctrl && !is_shift && !is_alt => {
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::ToggleWidePreview);
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
                // Leave Ctrl+Alt+Tab alone: it belongs to other features
                // (e.g. CycleByLetter's modifier family) and to the WM.
                if is_alt {
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
                // When either terminal holds focus, let Tab pass through
                // to its own EventControllerKey so the shell sees \t for completion.
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::NextExclusive);
                sender.input(AppMsg::TogglePreviewGridFocus);
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
            // Full-screen preview: Esc leaves it. The handler is a no-op otherwise and the key
            // is not consumed, so every other Esc behaviour keeps working.
            gdk::Key::Escape if !is_ctrl && !is_shift && !is_alt && !terminal_focused => {
                sender.input(AppMsg::ExitPreviewFullscreen);
                glib::Propagation::Proceed
            }
            _ => glib::Propagation::Proceed,
        }
    });

    window.add_controller(capture);
}
