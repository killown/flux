use crate::domain::model::{
    ActionsMsg, AppMsg, AppSidebarMsg, FileOpsMsg, FluxApp, IconsMsg, NavigationMsg, SearchMsg,
    TabsMsg, TagsMsg, TerminalMsg, UndoMsg, ViewMsg, WindowMsg,
};
use crate::ui::constants;
use crate::ui::keymap::KeyMap;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// Every shortcut registered via the resolved [`KeyMap`].
pub fn setup_global_shortcuts(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
    keymap: &KeyMap,
) {
    crate::hit!("setup_global_shortcuts");
    let sc = gtk::ShortcutController::new();
    sc.set_scope(gtk::ShortcutScope::Managed);

    // Helper to cut the boilerplate: register a trigger → AppMsg.
    //
    // The trigger is an `Option<gtk::ShortcutTrigger>` because `config.toml`
    // can set a shortcut to `"disabled"`, which `KeyMap` resolves to `None`.
    // A disabled shortcut must register nothing at all, not fall back to a
    // default: otherwise the chord would still fire.
    macro_rules! bind_msg {
        ($trigger:expr, $msg:expr) => {{
            if let Some(trigger) = $trigger.clone() {
                let s = sender.clone();
                let msg = $msg;
                sc.add_shortcut(gtk::Shortcut::new(
                    Some(trigger),
                    Some(gtk::CallbackAction::new(move |_, _| {
                        s.input(msg.clone());
                        glib::Propagation::Stop
                    })),
                ));
            }
        }};
    }

    bind_msg!(
        keymap.refresh.clone(),
        AppMsg::Navigation(NavigationMsg::Refresh)
    );
    bind_msg!(
        keymap.copy_path.clone(),
        AppMsg::FileOps(FileOpsMsg::CopyPath)
    );
    bind_msg!(
        keymap.create_symlink.clone(),
        AppMsg::FileOps(FileOpsMsg::CreateSymlink)
    );
    bind_msg!(
        keymap.create_hardlink.clone(),
        AppMsg::FileOps(FileOpsMsg::CreateHardlink)
    );
    bind_msg!(
        keymap.toggle_header.clone(),
        AppMsg::Window(WindowMsg::ToggleHeaderBar)
    );
    bind_msg!(
        keymap.toggle_hidden.clone(),
        AppMsg::View(ViewMsg::ToggleHidden)
    );
    bind_msg!(keymap.delete.clone(), AppMsg::FileOps(FileOpsMsg::Delete));
    bind_msg!(
        keymap.properties.clone(),
        AppMsg::FileOps(FileOpsMsg::TriggerRenameSelection)
    );
    bind_msg!(
        keymap.open.clone(),
        AppMsg::Navigation(NavigationMsg::Open {
            position: None,
            tab_id: None,
        })
    );
    bind_msg!(
        keymap.root.clone(),
        AppMsg::Navigation(NavigationMsg::Navigate(PathBuf::from("/")))
    );
    bind_msg!(
        keymap.change_icon.clone(),
        AppMsg::Icons(IconsMsg::TriggerIconPicker)
    );
    bind_msg!(
        keymap.reset_icon.clone(),
        AppMsg::Icons(IconsMsg::TriggerResetIcon)
    );
    bind_msg!(keymap.new_tab.clone(), AppMsg::Tabs(TabsMsg::NewTab(None)));
    bind_msg!(
        keymap.close_tab.clone(),
        AppMsg::Tabs(TabsMsg::CloseTab(None))
    );
    bind_msg!(keymap.next_tab.clone(), AppMsg::Tabs(TabsMsg::NextTab));
    bind_msg!(keymap.prev_tab.clone(), AppMsg::Tabs(TabsMsg::PrevTab));

    bind_msg!(
        keymap.open_debug_window.clone(),
        AppMsg::Window(WindowMsg::OpenDebugWindow)
    );
    bind_msg!(
        keymap.toggle_statusbar.clone(),
        AppMsg::View(ViewMsg::ToggleStatusBar)
    );
    bind_msg!(
        keymap.toggle_sidebar.clone(),
        AppMsg::Sidebar(AppSidebarMsg::ToggleSidebar)
    );
    bind_msg!(
        keymap.toggle_terminal.clone(),
        AppMsg::Terminal(TerminalMsg::ToggleTerminal)
    );
    bind_msg!(keymap.undo.clone(), AppMsg::Undo(UndoMsg::Undo));
    bind_msg!(keymap.redo.clone(), AppMsg::Undo(UndoMsg::Redo));
    bind_msg!(
        keymap.toggle_tag_panel.clone(),
        AppMsg::Tags(TagsMsg::ToggleTagPanel)
    );
    bind_msg!(
        keymap.toggle_actions_panel.clone(),
        AppMsg::Actions(ActionsMsg::ToggleActionsPanel)
    );
    bind_msg!(keymap.copy.clone(), AppMsg::FileOps(FileOpsMsg::Copy));
    bind_msg!(keymap.cut.clone(), AppMsg::FileOps(FileOpsMsg::Cut));
    bind_msg!(keymap.paste.clone(), AppMsg::FileOps(FileOpsMsg::Paste));
    bind_msg!(
        keymap.toggle_location_panel.clone(),
        AppMsg::Navigation(NavigationMsg::ToggleLocationPanel)
    );
    bind_msg!(keymap.cycle_sort.clone(), AppMsg::View(ViewMsg::CycleSort));
    bind_msg!(
        keymap.toggle_sort_order.clone(),
        AppMsg::View(ViewMsg::ToggleSortOrder)
    );

    window.add_controller(sc);

    // ── Grid/list toggle must work no matter where focus is ───────────────
    //
    // Capture phase runs before the focused widget (entries, sidebar, panels)
    // gets the key, so none of them can swallow it. The embedded terminals are
    // the exception: they keep every key while focused, so the callback lets
    // the chord through to them. Same `Option` rule as `bind_msg!`: "disabled"
    // resolves to `None` and registers nothing.
    if let Some(trigger) = keymap.toggle_list_mode.clone() {
        let list_sc = gtk::ShortcutController::new();
        list_sc.set_propagation_phase(gtk::PropagationPhase::Capture);
        let s = sender.clone();
        list_sc.add_shortcut(gtk::Shortcut::new(
            Some(trigger),
            Some(gtk::CallbackAction::new(move |widget, _| {
                // All three terminals (bottom, sidebar, fullscreen) carry this
                // class on their focusable `DrawingArea`.
                let terminal_focused = widget
                    .root()
                    .and_then(|root| root.focus())
                    .is_some_and(|f| f.has_css_class("flux-terminal-area"));
                if terminal_focused {
                    return glib::Propagation::Proceed;
                }

                s.input(AppMsg::View(ViewMsg::ToggleListMode));
                glib::Propagation::Stop
            })),
        ));
        window.add_controller(list_sc);
    }

    // ── Search has a special guard: don't steal focus during rename ───────
    //
    // The trigger is resolved through `parse_trigger_opt`, so `search =
    // "disabled"` skips the controller entirely instead of registering a
    // dead chord.
    if let Some(search_trigger) = keymap.search.clone() {
        let search_sc = gtk::ShortcutController::new();
        let s_search = sender.clone();
        search_sc.add_shortcut(gtk::Shortcut::new(
            Some(search_trigger),
            Some(gtk::CallbackAction::new(move |widget, _| {
                let rename_is_focused = widget
                    .root()
                    .and_then(|root| root.focus())
                    .map(|focused: gtk::Widget| {
                        focused
                            .css_classes()
                            .iter()
                            .any(|c: &gtk::glib::GString| c.as_str() == "flux-rename-entry")
                    })
                    .unwrap_or(false);

                if rename_is_focused {
                    return glib::Propagation::Proceed;
                }

                s_search.input(AppMsg::Search(SearchMsg::ToggleSearchPanel));
                s_search.input(AppMsg::Search(SearchMsg::SwitchHeader(
                    constants::VIEW_SEARCH.to_string(),
                )));
                glib::Propagation::Stop
            })),
        ));
        window.add_controller(search_sc);
    }
}

/// Settings window shortcut (needs the parent window for `transient_for`).
pub fn setup_settings_shortcut(window: &impl IsA<gtk::Widget>, keymap: &KeyMap) {
    crate::hit!("setup_settings_shortcut");
    let parent_window = window.root().and_downcast::<gtk::Window>();
    let sc = gtk::ShortcutController::new();
    sc.set_scope(gtk::ShortcutScope::Managed);

    // `settings = "disabled"` resolves to `None`, so the controller is never
    // installed and F10 (or whatever it was rebound to) does nothing.
    let Some(trigger) = keymap.settings.clone() else {
        return;
    };

    sc.add_shortcut(gtk::Shortcut::new(
        Some(trigger),
        Some(gtk::CallbackAction::new(move |_, _| {
            crate::ui::settings::open(parent_window.as_ref());
            glib::Propagation::Stop
        })),
    ));

    window.add_controller(sc);
}
