use crate::model::{AppMsg, FluxApp};
use crate::ui::keymap::KeyMap;
use adw::gdk;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// When a capture-phase binding must stand aside and let the focused widget
/// handle the key instead.
#[derive(Clone, Copy)]
enum Yield {
    /// Always consume the chord.
    Never,
    /// Defer to the embedded terminal when it holds focus.
    Terminal,
    /// Defer to the embedded terminal and to any focused editable widget.
    TerminalOrEditable,
}

/// A user-rebindable capture-phase chord resolved from the [`KeyMap`].
struct CaptureBinding {
    trigger: gtk::ShortcutTrigger,
    msg: AppMsg,
    yield_to: Yield,
}

/// Returns `true` when the key event currently dispatched by `ctrl` exactly
/// matches `trigger`.
///
/// # Arguments
///
/// * `ctrl` - The capture-phase key controller handling the event.
/// * `trigger` - The resolved shortcut trigger to test against.
fn trigger_matches(ctrl: &gtk::EventControllerKey, trigger: &gtk::ShortcutTrigger) -> bool {
    ctrl.current_event()
        .is_some_and(|event| trigger.trigger(&event, false) == gdk::KeyMatch::Exact)
}

/// Builds the capture-phase bindings backed by the resolved [`KeyMap`].
///
/// Every `KeyMap` field is an `Option`: a `None` means the user set the
/// shortcut to `"disabled"` in `config.toml`, so the binding is skipped
/// entirely rather than substituted with a default. A disabled chord
/// therefore falls through to the focused widget exactly as if it were
/// never registered.
///
/// # Arguments
///
/// * `keymap` - The resolved key map holding the user's (or default) triggers.
fn capture_bindings(keymap: &KeyMap) -> Vec<CaptureBinding> {
    let bind = |trigger: &Option<gtk::ShortcutTrigger>,
                msg: AppMsg,
                yield_to: Yield|
     -> Option<CaptureBinding> {
        let trigger = trigger.clone()?;
        Some(CaptureBinding {
            trigger,
            msg,
            yield_to,
        })
    };

    [
        // Ctrl+Alt+/ and Ctrl+Alt+\ folder cycle. Consumed at capture phase so the
        // ShortcutController (Ctrl+W = close tab, etc.) never sees the chord.
        bind(
            &keymap.cycle_by_letter_cancel,
            AppMsg::CycleByLetterCancel,
            Yield::TerminalOrEditable,
        ),
        bind(
            &keymap.cycle_by_letter_reanchor,
            AppMsg::CycleByLetterReanchor,
            Yield::TerminalOrEditable,
        ),
        bind(
            &keymap.focus_pane_left,
            AppMsg::FocusPaneLeft,
            Yield::Terminal,
        ),
        bind(
            &keymap.focus_pane_right,
            AppMsg::FocusPaneRight,
            Yield::Terminal,
        ),
        bind(
            &keymap.cycle_filter_mode,
            AppMsg::CycleFilterMode,
            Yield::Terminal,
        ),
        // Ctrl+Shift+A: toggle contextual-actions panel. Capture phase
        // so a focused widget inside the panel cannot pre-empt the chord.
        bind(
            &keymap.toggle_actions_panel,
            AppMsg::ToggleActionsPanel,
            Yield::Terminal,
        ),
        // Ctrl+Shift+L: toggle connect-to-server panel.
        bind(
            &keymap.toggle_connect_panel,
            AppMsg::ToggleConnectPanel,
            Yield::Terminal,
        ),
        // Ctrl+Shift+F: toggle the glob-pattern filter bar. Capture phase so
        // it fires even while the filter entry itself has focus, which is the
        // one place a user is most likely to press it to dismiss the bar.
        bind(
            &keymap.toggle_filter_bar,
            AppMsg::ToggleFilterBar,
            Yield::Terminal,
        ),
        // Ctrl+M: toggle ephemeral MIME filter on the selection. Moved
        // here for the same reason as Ctrl+Shift+A: it must work while
        // the peek-preview panel (or any other focused panel) is open.
        bind(
            &keymap.filter_by_mime,
            AppMsg::FilterBySelectedMime,
            Yield::TerminalOrEditable,
        ),
        bind(&keymap.toggle_header, AppMsg::ToggleHeaderBar, Yield::Never),
        bind(
            &keymap.toggle_folders_first,
            AppMsg::ToggleCurrentFoldersFirst,
            Yield::Never,
        ),
        bind(
            &keymap.toggle_statusbar,
            AppMsg::ToggleStatusBar,
            Yield::Never,
        ),
        bind(&keymap.toggle_sidebar, AppMsg::ToggleSidebar, Yield::Never),
        // F4 toggles the terminal panel. When either terminal has focus,
        // the terminal's own key controller fires first and handles this
        // key, so we yield there to avoid a double fire.
        bind(
            &keymap.toggle_terminal,
            AppMsg::ToggleTerminal,
            Yield::Terminal,
        ),
        // Wide preview: F11 is left to the shell when the terminal has focus.
        bind(
            &keymap.toggle_wide_preview,
            AppMsg::ToggleWidePreview,
            Yield::Terminal,
        ),
        // Tags: Ctrl + Shift + T
        bind(
            &keymap.toggle_tag_panel,
            AppMsg::ToggleTagPanel,
            Yield::Terminal,
        ),
        // Let the terminal shell handle Ctrl+Z (job control) if it has focus.
        bind(&keymap.undo, AppMsg::Undo, Yield::Terminal),
        bind(&keymap.redo, AppMsg::Redo, Yield::Terminal),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Capture-phase key controller. Handles shortcuts that must win over
/// focused children (Tab cycling, Ctrl+Z/Y, Insert/End/PageUp/PageDown,
/// F-keys, Ctrl+Shift+T/A/L, Ctrl+Alt+<letter> cycle, Ctrl+Insert Navigate
/// mode). Yields to editable
/// widgets and the terminal.
///
/// Rule of thumb: if a chord must work no matter which widget currently
/// has focus (including widgets inside right-side panels), it belongs
/// here, not in a window-level `ShortcutController`. Capture-phase
/// `EventControllerKey` runs before any focused child can pre-empt the
/// event, bubble-phase shortcuts do not.
pub fn setup_capture(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
    keymap: &KeyMap,
) {
    let bindings = capture_bindings(keymap);
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

        for binding in &bindings {
            if !trigger_matches(ctrl, &binding.trigger) {
                continue;
            }
            let yields = match binding.yield_to {
                Yield::Never => false,
                Yield::Terminal => terminal_focused,
                Yield::TerminalOrEditable => terminal_focused || is_editable,
            };
            if yields {
                return glib::Propagation::Proceed;
            }
            sender.input(binding.msg.clone());
            return glib::Propagation::Stop;
        }

        // Ctrl+Alt+<letter> folder cycle. Consumed at capture phase so the
        // ShortcutController (Ctrl+W = close tab, etc.) never sees the chord.
        if is_ctrl && is_alt && !is_shift && !is_editable && !terminal_focused {
            if let Some(c) = keyval.to_unicode() {
                let c = c.to_ascii_lowercase();
                if c.is_ascii_lowercase() {
                    sender.input(AppMsg::CycleByLetter(c));
                    return glib::Propagation::Stop;
                }
            }
        }

        // Every Quick List chord is claimed here, and only here. While the
        // terminal has focus `dispatch_key` returns `None`, so Tab, Insert,
        // Ctrl+End, Ctrl+PageUp/Down and Ctrl+1..9 all reach the shell.
        if let Some(chord) = crate::model::quick_list::dispatch_key(
            keyval,
            is_ctrl,
            is_shift,
            is_alt,
            terminal_focused,
        ) {
            use crate::model::quick_list::QuickKey;
            match chord {
                QuickKey::AddCurrent => sender.input(AppMsg::AddCurrentFolderExclusive),
                QuickKey::Clear => sender.input(AppMsg::ClearExclusive),
                QuickKey::Prev => sender.input(AppMsg::PrevExclusive),
                QuickKey::Next => sender.input(AppMsg::NextExclusive),
                QuickKey::Jump(n) => sender.input(AppMsg::QuickListJump(n)),
                QuickKey::TogglePanel => sender.input(AppMsg::ToggleQuickListPanel),
                QuickKey::Tab => {
                    sender.input(AppMsg::QuickListTab);
                    sender.input(AppMsg::TogglePreviewGridFocus);
                }
            }
            return glib::Propagation::Stop;
        }

        match keyval {
            gdk::Key::Insert if is_ctrl && is_shift => {
                sender.input(AppMsg::AddToSidebarPermanent);
                glib::Propagation::Stop
            }
            // Ctrl+Insert toggles Navigate mode. Text entries and the terminal
            // keep it (Ctrl+Insert is copy there).
            gdk::Key::Insert if is_ctrl => {
                if terminal_focused || is_editable {
                    return glib::Propagation::Proceed;
                }
                sender.input(AppMsg::ToggleNavigateMode);
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
