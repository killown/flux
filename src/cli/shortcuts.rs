use adw::gio;
use adw::prelude::*;
use relm4::prelude::*;

use crate::ui::keymap::constants;
use crate::ui::keymap::parse_accel_opt;

/// Installs an app-level accelerator action, honoring `shortcut = "disabled"`
/// in `config.toml`. When the config value is the `DISABLED` sentinel, no
/// accel is registered for this action, the action is still callable from
/// menus, but the key does nothing.
fn bind_accel<F>(
    app: &adw::Application,
    name: &'static str,
    user_val: &Option<String>,
    default: &str,
    activate: F,
) where
    F: Fn() + 'static,
{
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| activate());
    app.add_action(&action);

    if let Some(accel) = parse_accel_opt(user_val, default) {
        app.set_accels_for_action(&format!("app.{name}"), &[accel.as_str()]);
    }
}

pub(super) fn setup_shortcuts(app: &adw::Application) {
    let config = crate::utils::load_config();

    bind_accel(
        app,
        "open-menu-editor",
        &config.shortcuts.menu_editor,
        constants::MENU_EDITOR,
        {
            let app = app.clone();
            move || crate::ui::menu::editor::open(app.active_window().as_ref())
        },
    );

    bind_accel(
        app,
        "open-settings",
        &config.shortcuts.settings,
        constants::SETTINGS,
        {
            let app = app.clone();
            move || crate::ui::settings::open(app.active_window().as_ref())
        },
    );

    bind_accel(app, "open-help", &None, "F1", || {
        if let Some(s) = crate::model::SENDER.get() {
            let _ = s.send(crate::model::AppMsg::ShowHelp);
        }
    });

    bind_accel(
        app,
        "new-window",
        &config.shortcuts.new_tab,
        "<Primary>n",
        || {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).spawn();
            }
        },
    );
}
