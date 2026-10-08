use adw::gio;
use adw::prelude::*;
use relm4::prelude::*;

pub(super) fn setup_shortcuts(app: &adw::Application) {
    let action_menu_editor = gio::SimpleAction::new("open-menu-editor", None);
    action_menu_editor.connect_activate(|_, _| {
        let exe = std::env::current_exe().expect("Failed to get current exe path");
        let _ = std::process::Command::new(exe).arg("--menu-editor").spawn();
    });
    app.add_action(&action_menu_editor);
    app.set_accels_for_action("app.open-menu-editor", &["F9"]);

    let action_settings = gio::SimpleAction::new("open-settings", None);
    action_settings.connect_activate(|_, _| {
        let settings_win = crate::ui::SettingsWindow::builder().launch(()).detach();
        settings_win.widget().present();
    });
    app.add_action(&action_settings);
    app.set_accels_for_action("app.open-settings", &["F10"]);

    let action_help = gio::SimpleAction::new("open-help", None);
    action_help.connect_activate(|_, _| {
        if let Some(s) = crate::model::SENDER.get() {
            let _ = s.send(crate::model::AppMsg::ShowHelp);
        }
    });
    app.add_action(&action_help);
    app.set_accels_for_action("app.open-help", &["F1"]);

    let action_new_window = gio::SimpleAction::new("new-window", None);
    action_new_window.connect_activate(|_, _| {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
    });
    app.add_action(&action_new_window);
    app.set_accels_for_action("app.new-window", &["<Primary>n"]);
}
