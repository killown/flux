use crate::model::{AppMsg, Config, FluxApp};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

impl FluxApp {
    /// Creates the embedded terminal, its key/mouse controllers and shutdown hooks.
    pub(super) fn build_terminal(
        config: &Config,
        root: &adw::Window,
        sender: &AsyncComponentSender<Self>,
    ) -> crate::services::terminal::Terminal {
        let terminal = {
            crate::hit!("init_components:terminal");
            let terminal = crate::services::terminal::Terminal::new(&config.ui.terminal);
            terminal.connect_theme_changes(config.ui.terminal.clone());

            // Resolve theme colors on first map, when the widget has its real style context.
            let applied = std::rc::Rc::new(std::cell::Cell::new(false));
            let term_for_theme = terminal.clone();
            let term_config = config.ui.terminal.clone();
            terminal.drawing_area.connect_map(move |_| {
                if !applied.replace(true) {
                    term_for_theme.apply_theme(&term_config);
                }
            });
            terminal
        };

        {
            let s = sender.clone();
            terminal.set_cwd_callback(move |path| {
                s.input(AppMsg::TerminalCwdChanged(path));
            });
        }

        // Intercept F4 and clipboard shortcuts before terminal consumes them.
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
                        term_sender.input(AppMsg::ToggleTerminal);
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

        // Middle-click paste from the primary selection.
        {
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
