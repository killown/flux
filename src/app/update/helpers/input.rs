//! Keyboard, mouse and action wiring for the main window.

use crate::domain::model::{AppMsg, ActionsMsg, AppSidebarMsg, FileOpsMsg, NavigationMsg, PreviewMsg, QuickListMsg, SearchMsg, TabsMsg, ViewMsg, WindowMsg, FluxApp};
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;

/// Parses a string representation of a pointer button into its corresponding numeric identifier.
///
/// Maps standard identifiers (such as button/pointer names or back/forward aliases)
/// to their numerical button values (e.g., 8 for Back and 9 for Forward).
pub fn parse_mouse_button(raw: &str) -> Option<u32> {
    let s = raw.trim().to_lowercase();
    match s.as_str() {
        "button8" | "pointer8" | "mouse8" | "back" => Some(8),
        "button9" | "pointer9" | "mouse9" | "forward" => Some(9),
        "button2" | "pointer2" | "mouse2" | "middle" => Some(2),
        "button1" | "pointer1" | "mouse1" | "left" => Some(1),
        "button3" | "pointer3" | "mouse3" | "right" => Some(3),
        _ => None,
    }
}

// ── PEEK PREVIEW ──────────────────────────────────────────────────────────
/// Predicate for whether `Ctrl+Space` should toggle the peek-preview panel.
pub fn should_trigger_preview(
    keyval: gdk::Key,
    modifiers: gdk::ModifierType,
    is_editable: bool,
    terminal_focused: bool,
) -> bool {
    let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
    keyval == gdk::Key::space && ctrl && !is_editable && !terminal_focused
}

/// Predicate for whether bare `Space` should open the preview panel and
/// apply the selected item's MIME filter. Blocked in text entries and in the
/// embedded terminal, where Space has its own meaning.
pub fn should_trigger_preview_and_mime(
    keyval: gdk::Key,
    modifiers: gdk::ModifierType,
    is_editable: bool,
    terminal_focused: bool,
) -> bool {
    keyval == gdk::Key::space && modifiers.is_empty() && !is_editable && !terminal_focused
}
// ──────────────────────────────────────────────────────────────────────────

impl FluxApp {
    /// Processes keyboard events to trigger system shortcuts or manage focus-dependent navigation.
    ///
    /// Args:
    ///     ctrl: The key event controller.
    ///     keyval: The specific key pressed.
    ///     state: Bitmask of modifier keys (Ctrl, Alt, etc.).
    ///     sender: The component's message sender.
    ///     header_view: ID of the currently active header bar view.
    ///     terminal_focused: True when the embedded terminal's DrawingArea
    ///         holds keyboard focus. The terminal keeps `Space` (Page Down
    ///         in less/man/vim), so preview must not steal it.
    pub fn handle_key_event(
        ctrl: &gtk::EventControllerKey,
        keyval: gdk::Key,
        state: gdk::ModifierType,
        sender: &AsyncComponentSender<Self>,
        header_view: &str,
        terminal_focused: bool,
    ) -> glib::Propagation {
        crate::hit!("handle_key_event");
        let modifiers = state & gtk::accelerator_get_default_mod_mask();

        // System Keys
        if keyval == gdk::Key::F1 {
            sender.input(AppMsg::Window(WindowMsg::ShowHelp));
            return glib::Propagation::Stop;
        }

        if keyval == gdk::Key::F2 {
            sender.input(AppMsg::FileOps(FileOpsMsg::TriggerRenameSelection));
            return glib::Propagation::Stop;
        }

        if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            match keyval {
                gdk::Key::t | gdk::Key::T => {
                    sender.input(AppMsg::Tabs(TabsMsg::NewTab(None)));
                    return glib::Propagation::Stop;
                }
                gdk::Key::w | gdk::Key::W => {
                    sender.input(AppMsg::Tabs(TabsMsg::CloseTab(None)));
                    return glib::Propagation::Stop;
                }
                gdk::Key::Tab => {
                    if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                        sender.input(AppMsg::Tabs(TabsMsg::PrevTab));
                    } else {
                        sender.input(AppMsg::Tabs(TabsMsg::NextTab));
                    }
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
        }

        if modifiers == gdk::ModifierType::CONTROL_MASK && keyval == gdk::Key::Delete {
            sender.input(AppMsg::QuickList(QuickListMsg::ClearExclusive));
            return glib::Propagation::Stop;
        }

        // Ctrl+Insert toggles Navigate mode. It is decided further down, once
        // the terminal / editable yield is known, so it never eats the
        // clipboard-copy meaning Ctrl+Insert has inside a text entry.
        if keyval == gdk::Key::Insert && modifiers != gdk::ModifierType::CONTROL_MASK {
            if modifiers == gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK {
                sender.input(AppMsg::Sidebar(AppSidebarMsg::AddToSidebarPermanent));
            } else {
                sender.input(AppMsg::QuickList(QuickListMsg::AddCurrentFolderExclusive));
            }
            return glib::Propagation::Stop;
        }

        // Yield every remaining key to the embedded terminal. The window-level
        // capture controller runs before the terminal's own controller, so
        // without this the type-to-search branch below would steal plain
        // letters and redirect focus to the search entry.
        if terminal_focused {
            return glib::Propagation::Proceed;
        }

        // Focus Bypass Check (The Search Logic)
        let is_editable = ctrl
            .widget()
            .and_then(|w| w.root())
            .and_then(|r| r.focus())
            .map(|f| f.type_().is_a(gtk::Editable::static_type()))
            .unwrap_or(false);

        // If any text entry in the window is focused (rename entry, search, etc.),
        // do not steal Return, Escape, or typing!
        if is_editable {
            if (header_view == "search" || header_view == "entry") && keyval == gdk::Key::Escape {
                sender.input(AppMsg::Search(SearchMsg::CancelContentSearch));
                sender.input(AppMsg::Search(SearchMsg::UpdateFilter(String::new())));
                sender.input(AppMsg::Search(SearchMsg::SwitchHeader("path".to_string())));
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }

        if keyval == gdk::Key::Insert && modifiers == gdk::ModifierType::CONTROL_MASK {
            sender.input(AppMsg::Navigation(NavigationMsg::ToggleNavigateMode));
            return glib::Propagation::Stop;
        }

        // ── PEEK PREVIEW ─────────────────────────────────────────────────
        // Space toggles the peek-preview panel. Guarded by `is_editable`
        // above and by `terminal_focused`, so typing a space in the search
        // bar, rename entry, location entry, or any other Entry/SearchEntry
        // never reaches this branch, and the embedded terminal keeps its
        // own Space (Page Down in less/man/vim). Placed before type-to-search
        // because Space is not a search-starting character.
        if should_trigger_preview(keyval, modifiers, is_editable, terminal_focused) {
            sender.input(AppMsg::Preview(PreviewMsg::TogglePreview));
            return glib::Propagation::Stop;
        }

        // Bare Space opens the preview panel and applies the selected
        // item's MIME filter. Only fires when nothing is already filtered
        // by MIME, so pressing Space again does not clear it.
        if should_trigger_preview_and_mime(keyval, modifiers, is_editable, terminal_focused) {
            sender.input(AppMsg::Search(SearchMsg::TogglePreviewAndFilterBySelectedMime));
            return glib::Propagation::Stop;
        }
        // ─────────────────────────────────────────────────────────────────

        // Global Capture (Type-to-search)
        // Only triggers when no modifiers (like Ctrl or Shift) are held,
        // allowing modifiers to be used for native multi-selection expansion.
        // Allow unmodified keys (letters/numbers) AND Shift+key (for symbols)
        let forbidden = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK;

        if !modifiers.intersects(forbidden) {
            if let Some(c) = keyval.to_unicode() {
                // Allow alphanumerics, tags (#), content search (:), and size filters (<)
                if c.is_ascii_alphanumeric() || c == ':' || c == '<' || c == '#' {
                    // Type-to-search, or a Navigate-mode prefix jump: the update
                    // handler knows which, so the key controller stays stateless.
                    sender.input(AppMsg::Navigation(NavigationMsg::KeyChar(c)));
                    return glib::Propagation::Stop;
                }
            }
        }

        // Context Keys & Batch Execution
        match keyval {
            // Triggers batch activation for the current selection
            gdk::Key::Return | gdk::Key::KP_Enter => {
                sender.input(AppMsg::Navigation(NavigationMsg::KeyEnter));
                glib::Propagation::Stop
            }
            gdk::Key::F2 => {
                sender.input(AppMsg::FileOps(FileOpsMsg::TriggerRenameSelection));
                glib::Propagation::Stop
            }
            gdk::Key::Escape => {
                sender.input(AppMsg::Navigation(NavigationMsg::KeyEscape));
                glib::Propagation::Stop
            }
            gdk::Key::BackSpace => {
                sender.input(AppMsg::Navigation(NavigationMsg::KeyBackspace));
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    /// Attaches a motion controller to change the cursor to a pointer on hover.
    /// We use concrete &gtk::Widget here to bypass trait resolution conflicts.
    pub fn set_cursor_pointer(widget: &gtk::Widget, enabled: bool) {
        if !enabled {
            return;
        }

        let controller = gtk::EventControllerMotion::new();

        controller.connect_enter(|ctrl, _, _| {
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor_from_name(Some("pointer"));
            }
        });

        controller.connect_leave(|ctrl| {
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor(None);
            }
        });

        widget.add_controller(controller);
    }

    /// Registers application-wide keyboard shortcuts with a ShortcutController.
    ///
    /// Chords that must work regardless of which widget currently has focus
    /// (e.g. `Ctrl+Shift+A`, `Ctrl+Shift+L`, `Ctrl+M`) are handled at capture
    /// phase in `src/ui/inputs/capture.rs`, not here. A window-level
    /// `ShortcutController` at Global scope is pre-empted by any focused
    /// widget that installs its own key controller, which is exactly what
    /// used to break panel-toggle chords once the user clicked inside a
    /// panel.
    ///
    /// `Delete` is deliberately **not** registered here. It is a
    /// configurable shortcut (`[shortcuts].delete`), wired through
    /// `KeyMap` and `ui/inputs/shortcuts.rs`. A hardcoded copy in this
    /// controller would bypass the keymap, so `delete = "disabled"` and a
    /// rebind to another chord would both keep firing on a bare `Delete`.
    pub fn setup_shortcuts(
        controller: &gtk::ShortcutController,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("setup_shortcuts");
        let shortcuts = [
            ("F1", AppMsg::Window(WindowMsg::ShowHelp)),
            ("<Shift>s", AppMsg::View(ViewMsg::CycleFolderPriority)),
        ];

        for (trigger_str, msg) in shortcuts {
            let s_clone = sender.clone();
            controller.add_shortcut(gtk::Shortcut::new(
                Some(gtk::ShortcutTrigger::parse_string(trigger_str).unwrap()),
                Some(gtk::CallbackAction::new(move |_, _| {
                    s_clone.input(msg.clone());
                    glib::Propagation::Stop
                })),
            ));
        }

        // Handle specific string-argument messages separately
        let f_sender = sender.clone();
        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::ShortcutTrigger::parse_string("<Control>f").unwrap()),
            Some(gtk::CallbackAction::new(move |_, _| {
                f_sender.input(AppMsg::Search(SearchMsg::ToggleSearchPanel));
                glib::Propagation::Stop
            })),
        ));

        // NOTE: `Space` is intentionally NOT registered here. ShortcutController
        // at Global scope fires regardless of which widget has focus, which
        // would open the preview when the user types a space into the search
        // bar. The binding lives in `handle_key_event` instead, guarded by
        // the `is_editable` check.
    }

    /// Registers GIO actions to the application's internal action group.
    pub fn setup_actions(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("setup_actions");
        // Folder Priority Action
        let prio_action = gio::SimpleAction::new("cycle-priority", None);
        let prio_sender = sender.clone();
        prio_action.connect_activate(move |_, _| {
            prio_sender.input(AppMsg::View(ViewMsg::CycleFolderPriority));
        });
        self.action_group.add_action(&prio_action);

        // Clipboard: Copy Action
        let copy_action = gio::SimpleAction::new("copy", None);
        let c_sender = sender.clone();
        copy_action.connect_activate(move |_, _| {
            c_sender.input(AppMsg::FileOps(FileOpsMsg::Copy));
        });
        self.action_group.add_action(&copy_action);

        // Clipboard: Cut Action
        let cut_action = gio::SimpleAction::new("cut", None);
        let x_sender = sender.clone();
        cut_action.connect_activate(move |_, _| {
            x_sender.input(AppMsg::FileOps(FileOpsMsg::Cut));
        });
        self.action_group.add_action(&cut_action);

        // Clipboard: Paste Action
        let paste_action = gio::SimpleAction::new("paste", None);
        let v_sender = sender.clone();
        paste_action.connect_activate(move |_, _| {
            v_sender.input(AppMsg::FileOps(FileOpsMsg::Paste));
        });
        self.action_group.add_action(&paste_action);

        // Parameterized "Open With..." Action
        let launch_action =
            gio::SimpleAction::new("launch-with", Some(glib::VariantTy::new("s").unwrap()));
        let launch_sender = sender.clone();
        launch_action.connect_activate(move |_, parameter| {
            if let Some(app_id) = parameter.and_then(|p| p.get::<String>()) {
                launch_sender.input(AppMsg::Actions(ActionsMsg::LaunchWithApp(app_id)));
            }
        });
        self.action_group.add_action(&launch_action);

        // Quick List Direct Transfers (Move and Copy Actions)
        self.setup_quick_transfer_actions(sender);

        // Dynamic Context Menu Actions (Shell Commands)
        for action_def in &self.menu_actions {
            // Skip builtins as they are handled by the explicit actions above
            if action_def.command.starts_with("builtin::") {
                continue;
            }

            let cmd_clone = action_def.command.clone();
            let sender_clone = sender.clone();
            let action = gio::SimpleAction::new(&action_def.action_name, None);
            action.connect_activate(move |_, _| {
                sender_clone.input(AppMsg::Actions(ActionsMsg::ExecuteCommand(cmd_clone.clone())));
            });
            self.action_group.add_action(&action);
        }
    }
}
