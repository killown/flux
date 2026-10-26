use crate::infra::terminal::Terminal;
use crate::model::{AppMsg, Config, FluxApp, TerminalConfig};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

/// Applies the theme to `terminal` now-ish: follows theme changes and resolves the
/// colors once on first map, when the widget has its real style context.
pub(crate) fn wire_terminal_theme(terminal: &Terminal, config: &TerminalConfig) {
    terminal.connect_theme_changes(config.clone());

    let applied = std::rc::Rc::new(std::cell::Cell::new(false));
    let term_for_theme = terminal.clone();
    let term_config = config.clone();
    terminal.drawing_area.connect_map(move |_| {
        if !applied.replace(true) {
            term_for_theme.apply_theme(&term_config);
        }
    });
}

/// Intercepts F4 and the clipboard shortcuts before the terminal consumes them.
///
/// `f4_msg` is what F4 dispatches. The bottom and sidebar terminals pass
/// `ToggleTerminal`, which the dispatcher in `update/network.rs` routes by the
/// configured position; the full-window terminal passes `ToggleTerminalFullscreen`.
pub(crate) fn attach_terminal_key_controller(
    terminal: &Terminal,
    sender: &AsyncComponentSender<FluxApp>,
    f4_msg: AppMsg,
) {
    let term_sender = sender.clone();
    let term_key_ctrl = gtk::EventControllerKey::new();
    term_key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let terminal_ref = terminal.clone();
        term_key_ctrl.connect_key_pressed(move |_, keyval, _, modifiers| {
            use gtk::gdk::Key;

            let ctrl_shift =
                gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK;

            match keyval {
                Key::F4 => {
                    term_sender.input(f4_msg.clone());
                    return glib::Propagation::Stop;
                }
                // Ctrl+Shift+C → copy selection to clipboard
                Key::c | Key::C if modifiers == ctrl_shift => {
                    terminal_ref.emit_copy_clipboard();
                    return glib::Propagation::Stop;
                }
                // Ctrl+Shift+V → paste from clipboard into terminal
                Key::v | Key::V if modifiers == ctrl_shift => {
                    terminal_ref.emit_paste_clipboard();
                    return glib::Propagation::Stop;
                }
                // Feed Tab directly to the PTY so shell completion works.
                Key::Tab if modifiers.is_empty() => {
                    terminal_ref.feed_child(b"\t");
                    return glib::Propagation::Stop;
                }
                _ => {}
            }

            glib::Propagation::Proceed
        });
    }
    terminal.add_controller(&term_key_ctrl);
}

/// Suppresses the right-click menu and pastes the primary selection on middle click.
pub(crate) fn attach_terminal_mouse_controllers(terminal: &Terminal) {
    // Suppress right-click menu.
    let right_click = gtk::GestureClick::new();
    right_click.set_button(3);
    right_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    right_click.connect_pressed(|gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
    });
    terminal.add_controller(&right_click);

    let terminal_ref = terminal.clone();
    let middle_click = gtk::GestureClick::new();
    middle_click.set_button(2); // BUTTON_MIDDLE
    middle_click.set_propagation_phase(gtk::PropagationPhase::Capture);
    middle_click.connect_pressed(move |gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        terminal_ref.emit_paste_clipboard();
    });
    terminal.add_controller(&middle_click);
}

/// Kills `terminal`'s shell when the window containing `anchor` closes and when the
/// application shuts down. For terminals created lazily, after `setup_window`.
pub(crate) fn register_terminal_shutdown(terminal: &Terminal, anchor: &impl IsA<gtk::Widget>) {
    if let Some(window) = anchor.root().and_downcast::<gtk::Window>() {
        let term_for_close = terminal.clone();
        window.connect_close_request(move |_| {
            term_for_close.kill_shell();
            glib::Propagation::Proceed
        });
    }

    let term_for_shutdown = terminal.clone();
    relm4::main_adw_application().connect_shutdown(move |_| {
        term_for_shutdown.kill_shell();
    });
}

impl FluxApp {
    /// Creates the embedded terminal, its key/mouse controllers and shutdown hooks.
    pub(super) fn build_terminal(
        config: &Config,
        root: &adw::Window,
        sender: &AsyncComponentSender<Self>,
    ) -> Terminal {
        let terminal = {
            crate::hit!("init_components:terminal");
            let terminal = Terminal::new(&config.ui.terminal);
            wire_terminal_theme(&terminal, &config.ui.terminal);
            terminal
        };

        {
            let s = sender.clone();
            terminal.set_cwd_callback(move |path| {
                s.input(AppMsg::TerminalCwdChanged(path));
            });
        }

        attach_terminal_key_controller(&terminal, sender, AppMsg::ToggleTerminal);
        attach_terminal_mouse_controllers(&terminal);

        let term_for_shutdown = terminal.clone();
        root.connect_close_request(move |_| {
            term_for_shutdown.kill_shell();
            glib::Propagation::Proceed
        });

        let app = relm4::main_adw_application();
        let term_for_app_shutdown = terminal.clone();
        app.connect_shutdown(move |_| {
            term_for_app_shutdown.kill_shell();
        });
        terminal
    }
}
