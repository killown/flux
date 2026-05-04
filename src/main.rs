mod model;
mod services;
mod ui;
mod utils;

use crate::model::{AppMsg, Config, FluxApp};
use crate::ui::FileProperties;
use adw::prelude::*;
use adw::{gio, glib};
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

    if let Some(data) = css_data {
        CSS_PROVIDER.with(|provider| {
            if let Some(display) = adw::gdk::Display::default() {
                gtk::style_context_remove_provider_for_display(&display, provider);
                provider.load_from_data(&data);
                gtk::style_context_add_provider_for_display(
                    &display,
                    provider,
                    gtk::STYLE_PROVIDER_PRIORITY_USER,
                );
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

/// Logs a warning for each optional external binary that is not found in `$PATH`.
fn check_optional_deps() {
    for bin in ["ffmpeg", "ffprobe", "magick"] {
        if std::process::Command::new("which")
            .arg(bin)
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            eprintln!("[flux] optional dependency '{bin}' not found in PATH");
        }
    }
}

fn setup_shortcuts(app: &adw::Application) {
    let action_menu_editor = gio::SimpleAction::new("open-menu-editor", None);
    action_menu_editor.connect_activate(|_, _| {
        let exe = std::env::current_exe().expect("Failed to get current exe path");
        let _ = std::process::Command::new(exe).arg("--menu-editor").spawn();
    });
    app.add_action(&action_menu_editor);
    app.set_accels_for_action("app.open-menu-editor", &["F9"]);
}

fn main() {
    adw::init().expect("Failed to initialize Libadwaita");

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" => {
                println!("flux {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                println!("Usage: flux [OPTIONS] [PATH]");
                println!();
                println!("Arguments:");
                println!("  [PATH]  Directory to open on startup");
                println!();
                println!("Options:");
                println!("  -h, --help                  Print this help message");
                println!("  -v, --version               Print version information");
                println!("      --file-properties PATH  Open the file properties window for PATH");
                println!("      --menu-editor           To manage menu.rs");
                return;
            }
            "--file-properties" if args.len() > 2 => {
                let path = PathBuf::from(&args[2]);
                let app = RelmApp::new("flux.PropertiesViewer");
                app.allow_multiple_instances(true);
                app.with_args(vec![]).run::<FileProperties>(path);
                return;
            }
            "--menu-editor" => {
                crate::ui::menu_editor::run();
                return;
            }
            arg if arg.starts_with('-') => {
                eprintln!("flux: unrecognized option '{arg}'");
                eprintln!("Try 'flux --help' for more information.");
                std::process::exit(1);
            }
            _ => {}
        }
    }

    glib::idle_add_local_once(|| {
        load_custom_css();
        setup_config_watcher();
        std::thread::spawn(check_optional_deps);
    });

    // --- CLI HANDLER: FILE PROPERTIES ---
    if args.len() > 2 && args[1] == "--file-properties" {
        // We use the new ui:: path here
        let path = PathBuf::from(&args[2]);
        let app = RelmApp::new("flux.PropertiesViewer");
        app.allow_multiple_instances(true);
        app.with_args(vec![]).run::<FileProperties>(path);
        return;
    }

    // --- MAIN APP HANDLER ---
    //NOTE: This must be not included here
    // Setting application_id in the builder triggers a synchronous D-Bus handshake
    // and Wayland compositor lookup that blocks the main thread for around ~200ms.
    let base_app = adw::Application::builder()
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    setup_shortcuts(&base_app);

    let app: RelmApp<AppMsg> = RelmApp::from_app(base_app);
    app.allow_multiple_instances(true);

    let start_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        dirs::home_dir().unwrap_or(PathBuf::from("."))
    };

    app.with_args(vec![]).run_async::<FluxApp>(start_path);
}
