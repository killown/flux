use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::ui::keymap::KeyMap;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// Every shortcut registered via the resolved [`KeyMap`] plus the handful of
/// hardcoded ones (F-keys, debug window, tags fallback, undo/redo).
pub fn setup_global_shortcuts(
    window: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
    keymap: &KeyMap,
) {
    let sc = gtk::ShortcutController::new();
    sc.set_scope(gtk::ShortcutScope::Global);

    // Helper to cut the boilerplate: register a trigger → AppMsg.
    macro_rules! bind_msg {
        ($trigger:expr, $msg:expr) => {{
            let s = sender.clone();
            let msg = $msg;
            sc.add_shortcut(gtk::Shortcut::new(
                Some($trigger),
                Some(gtk::CallbackAction::new(move |_, _| {
                    s.input(msg.clone());
                    glib::Propagation::Stop
                })),
            ));
        }};
    }

    macro_rules! bind_trigger {
        ($lit:literal, $msg:expr) => {
            bind_msg!(gtk::ShortcutTrigger::parse_string($lit).unwrap(), $msg)
        };
    }

    // ── KeyMap-driven ─────────────────────────────────────────────────────
    bind_msg!(keymap.refresh.clone(), AppMsg::Refresh);
    bind_msg!(keymap.copy_path.clone(), AppMsg::CopyPath);
    bind_msg!(keymap.create_symlink.clone(), AppMsg::CreateSymlink);
    bind_msg!(keymap.create_hardlink.clone(), AppMsg::CreateHardlink);
    bind_msg!(keymap.toggle_header.clone(), AppMsg::ToggleHeaderBar);
    bind_msg!(keymap.toggle_hidden.clone(), AppMsg::ToggleHidden);
    bind_msg!(keymap.delete.clone(), AppMsg::Delete);
    bind_msg!(keymap.properties.clone(), AppMsg::TriggerRenameSelection);
    bind_msg!(keymap.open.clone(), AppMsg::Open(None));
    bind_msg!(keymap.root.clone(), AppMsg::Navigate(PathBuf::from("/")));
    bind_msg!(keymap.change_icon.clone(), AppMsg::TriggerIconPicker);
    bind_msg!(keymap.reset_icon.clone(), AppMsg::TriggerResetIcon);
    bind_msg!(keymap.new_tab.clone(), AppMsg::NewTab(None));
    bind_msg!(keymap.close_tab.clone(), AppMsg::CloseTab(None));
    bind_msg!(keymap.next_tab.clone(), AppMsg::NextTab);
    bind_msg!(keymap.prev_tab.clone(), AppMsg::PrevTab);

    // ── Hardcoded F-keys / debug / tags ───────────────────────────────────
    bind_trigger!("<Primary><Shift>F7", AppMsg::OpenDebugWindow);
    bind_trigger!("F7", AppMsg::ToggleStatusBar);
    bind_trigger!("F8", AppMsg::ToggleSidebar);
    bind_trigger!("F4", AppMsg::ToggleTerminal);
    bind_trigger!("<Primary>z", AppMsg::Undo);
    bind_trigger!("<Primary><Shift>z", AppMsg::Redo);
    bind_trigger!("<Primary>y", AppMsg::Redo);
    bind_trigger!("<Primary><Shift>t", AppMsg::ToggleTagPanel);

    window.add_controller(sc);

    // ── Search has a special guard: don't steal focus during rename ───────
    let search_sc = gtk::ShortcutController::new();
    let s_search = sender.clone();
    let search_trigger = keymap.search.clone();
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

            s_search.input(AppMsg::ToggleSearchPanel);
            s_search.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
            glib::Propagation::Stop
        })),
    ));
    window.add_controller(search_sc);
}

/// Settings window shortcut (needs the parent window for `transient_for`).
pub fn setup_settings_shortcut(window: &impl IsA<gtk::Widget>, keymap: &KeyMap) {
    let parent_window = window.root().and_downcast::<gtk::Window>();
    let sc = gtk::ShortcutController::new();
    let trigger = keymap.settings.clone();

    sc.add_shortcut(gtk::Shortcut::new(
        Some(trigger),
        Some(gtk::CallbackAction::new(move |_, _| {
            let controller = crate::ui::SettingsWindow::builder().launch(());
            if let Some(parent) = &parent_window {
                controller.widget().set_transient_for(Some(parent));
            }
            controller.widget().present();
            controller.detach();
            glib::Propagation::Stop
        })),
    ));

    window.add_controller(sc);
}
