mod i18n;
mod model;
mod services;
mod ui;
mod utils;

use crate::model::{AppMsg, Config, FluxApp};
use crate::ui::FileProperties;
use adw::prelude::*;
use adw::{gio, glib};
use flux::args::{resolve_startup_action, StartupAction};
use relm4::prelude::*;
use std::cell::OnceCell;
use std::fs;
use std::path::PathBuf;

thread_local! {
    static CSS_PROVIDER: gtk::CssProvider = gtk::CssProvider::new();
    static CONFIG_MONITOR: OnceCell<gio::FileMonitor> = const { OnceCell::new() };
}

/// Loads CSS based on config.toml theme selection with local and internal fallbacks.
fn load_custom_css() {
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");
    let config_path = config_dir.join("config.toml");

    let config: Option<Config> = fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok());

    let mut css_data = None;

    if let Some(theme_name) = config.and_then(|c| c.ui.theme) {
        let theme_filename = format!("{}.css", theme_name);

        let local_theme = dirs::data_dir()
            .unwrap_or_default()
            .join("flux")
            .join("themes")
            .join(&theme_filename);

        let system_theme = PathBuf::from("/usr/share/flux/themes").join(&theme_filename);

        css_data = fs::read_to_string(&local_theme)
            .or_else(|_| fs::read_to_string(&system_theme))
            .ok();
    }

    if css_data.is_none() {
        css_data = fs::read_to_string(config_dir.join("style.css")).ok();
    }

    if let Some(display) = adw::gdk::Display::default() {
        CSS_PROVIDER.with(|provider| {
            // Remove existing provider from display
            gtk::style_context_remove_provider_for_display(&display, provider);

            if let Some(data) = css_data {
                // Load new CSS data
                provider.load_from_data(&data);

                // Add the updated provider back
                gtk::style_context_add_provider_for_display(
                    &display,
                    provider,
                    gtk::STYLE_PROVIDER_PRIORITY_USER,
                );
            } else {
                // No CSS data available, clear the provider
                provider.load_from_data("");
            }
        });
    }
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
        gio::FileMonitorFlags::WATCH_MOUNTS | gio::FileMonitorFlags::SEND_MOVED,
        gio::Cancellable::NONE,
    ) else {
        return;
    };

    monitor.connect_changed(|_, file, _, event_type| {
        if let Some(name) = file.basename() {
            let n = name.to_string_lossy();
            if n == "style.css" || n == "config.toml" {
                match event_type {
                    gio::FileMonitorEvent::Changed
                    | gio::FileMonitorEvent::ChangesDoneHint
                    | gio::FileMonitorEvent::Created
                    | gio::FileMonitorEvent::MovedIn => {
                        load_custom_css();
                        // Signal the application to reload the sidebar if the config file was modified
                        if n == "config.toml" {
                            if let Some(app) = gio::Application::default() {
                                app.activate_action("reload-sidebar", None);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    // `set` is a no-op if already initialized, the losing monitor is dropped,
    // which cancels the underlying GFileMonitor via GIO's ref-counting.
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
        load_custom_css();
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
