mod i18n;
mod model;
mod services;
mod ui;
mod utils;

use crate::model::{AppMsg, FluxApp};
use crate::ui::FileProperties;
use adw::prelude::*;
use adw::{gio, glib};
use flux::args::{resolve_startup_action, StartupAction};
use relm4::prelude::*;
use std::cell::OnceCell;
use std::path::PathBuf;

thread_local! {
    static CONFIG_MONITOR: OnceCell<gio::FileMonitor> = const { OnceCell::new() };
}

/// Sets up a GIO directory monitor to watch for config or style changes and refreshes UI components.
///
/// Idempotent: subsequent calls are no-ops. The monitor is stored in the thread-local
/// [`CONFIG_MONITOR`] and remains active for the entire process lifetime, which is the
/// correct scope for a config-directory watcher. Re-entrant calls (e.g. from a UI
/// restart) will not create a second monitor or cause a memory leak.
fn setup_config_watcher() {
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");
    let file = gio::File::for_path(&config_dir);

    let Ok(monitor) = file.monitor_directory(
        gio::FileMonitorFlags::WATCH_MOVES | gio::FileMonitorFlags::WATCH_MOUNTS,
        gio::Cancellable::NONE,
    ) else {
        return;
    };

    monitor.connect_changed(|_, file, other_file, event_type| {
        use gio::FileMonitorEvent::*;

        let file_name = file.basename().map(|n| n.to_string_lossy().to_string());
        let other_name = other_file
            .and_then(|f| f.basename())
            .map(|n| n.to_string_lossy().to_string());

        let matched = matches!(
            (file_name.as_deref(), other_name.as_deref()),
            (Some("config.toml" | "style.css"), _) | (_, Some("config.toml" | "style.css"))
        );

        if matched {
            match event_type {
                Changed | ChangesDoneHint | Created | MovedIn | Renamed | Moved => {
                    glib::timeout_add_local_once(std::time::Duration::from_millis(50), || {
                        crate::utils::helpers::load_custom_css();
                        if let Some(app) = gio::Application::default() {
                            app.activate_action("reload-sidebar", None);
                        }
                    });
                }
                _ => {}
            }
        }
    });

    CONFIG_MONITOR.with(|cell| {
        let _ = cell.set(monitor);
    });
}

fn setup_shortcuts(app: &adw::Application) {
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

fn launch_main_app(start_path: PathBuf, open_archive: Option<PathBuf>) {
    // Defer non-critical CSS/Theme loading and dependency checks by 150ms
    glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
        crate::utils::helpers::load_custom_css();
        setup_config_watcher();
        std::thread::spawn(crate::utils::deps::check_optional_deps);
        glib::ControlFlow::Break
    });

    // --- MAIN APP HANDLER ---
    //NOTE:
    // Setting application_id with .flags(gio::ApplicationFlags...) in the builder
    // triggers a synchronous D-Bus handshake  and Wayland compositor lookup
    // that blocks the main thread for around ~200ms.
    let base_app = adw::Application::builder().build();

    assert!(
        base_app.application_id().is_none(),
        "\n\n[flux] STARTUP REGRESSION: application_id is set on the main adw::Application.\n\
     This triggers a synchronous D-Bus name acquisition and Wayland compositor\n\
     lookup on the main thread, adding ~200ms to startup time.\n\
     Remove .application_id(...) from the adw::Application::builder() call.\n"
    );

    assert!(
        !base_app.flags().contains(gio::ApplicationFlags::NON_UNIQUE),
        "\n\n[flux] STARTUP REGRESSION: NON_UNIQUE flag is set on the main adw::Application.\n\
     This triggers a synchronous D-Bus handshake and Wayland compositor lookup\n\
     on the main thread, adding ~200ms to startup time.\n\
     Remove .flags(gio::ApplicationFlags::NON_UNIQUE) from the adw::Application::builder() call.\n"
    );

    setup_shortcuts(&base_app);

    if let Some(archive_path) = open_archive {
        std::env::set_var("FLUX_OPEN_ARCHIVE", &archive_path);
    }

    let app: RelmApp<AppMsg> = RelmApp::from_app(base_app);
    app.allow_multiple_instances(true);
    app.with_args(vec![]).run_async::<FluxApp>(start_path);
}

fn main() {
    i18n::init();
    adw::init().expect("Failed to initialize Libadwaita");

    let args: Vec<String> = std::env::args().collect();
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    match resolve_startup_action(&args, home_dir) {
        StartupAction::PrintVersion => {
            println!("flux {}", env!("CARGO_PKG_VERSION"));
        }

        StartupAction::PrintHelp => {
            println!("Usage: flux [OPTIONS] [PATH]");
            println!();
            println!("Arguments:");
            println!("  [PATH]  Directory or archive file to open on startup");
            println!();
            println!("Options:");
            println!("  -h, --help                  Print this help message");
            println!("  -v, --version               Print version information");
            println!("      --file-properties PATH  Open the file properties window for PATH");
            println!("      --menu-editor           To manage menu.rs");
        }

        StartupAction::FileProperties(path) => {
            let app = RelmApp::new("flux.PropertiesViewer");
            app.allow_multiple_instances(true);
            app.with_args(vec![]).run::<FileProperties>(path);
        }

        StartupAction::MenuEditor => {
            crate::ui::menu_editor::run();
        }

        StartupAction::UnknownFlag(flag) => {
            eprintln!("flux: unrecognized option '{flag}'");
            eprintln!("Try 'flux --help' for more information.");
            std::process::exit(1);
        }

        StartupAction::OpenArchive(archive_path) => {
            let start_path = archive_path.parent().unwrap_or(&archive_path).to_path_buf();
            launch_main_app(start_path, Some(archive_path));
        }

        StartupAction::Launch(start_path) => {
            launch_main_app(start_path, None);
        }
    }
}
