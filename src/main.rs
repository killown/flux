mod app;
mod ui_components;
mod utils;
mod model;
mod file_properties;

use relm4::prelude::*;
use crate::model::{FluxApp, Config};
use crate::file_properties::FileProperties;
use std::path::PathBuf;
use adw::prelude::*;
use adw::gio;
use std::fs;

// Thread-local provider allows load_from_data to overwrite previous styles on the same object
thread_local! {
    static CSS_PROVIDER: gtk::CssProvider = gtk::CssProvider::new();
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

        // 1. Try local user path: ~/.local/share/flux/themes/
        let local_theme = dirs::data_dir()
            .unwrap_or_default()
            .join("flux")
            .join("themes")
            .join(&theme_filename);

        // 2. Try system path: /usr/share/flux/themes/
        let system_theme = PathBuf::from("/usr/share/flux/themes").join(&theme_filename);

        css_data = fs::read_to_string(&local_theme)
            .or_else(|_| fs::read_to_string(&system_theme))
            .ok();
    }

    // Fallback to style.css in config dir if no theme found or loaded
    if css_data.is_none() {
        css_data = fs::read_to_string(config_dir.join("style.css")).ok();
    }

    if let Some(data) = css_data {
        CSS_PROVIDER.with(|provider| {
            provider.load_from_data(&data);
            if let Some(display) = adw::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    provider,
                    gtk::STYLE_PROVIDER_PRIORITY_USER,
                );
            }
        });
    }
}

/// Sets up a GIO directory monitor to watch for config or style changes
fn setup_css_watcher() {
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");
    let file = gio::File::for_path(&config_dir);

    if let Ok(monitor) = file.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        monitor.connect_changed(|_, file, _, event_type| {
            if let Some(name) = file.basename() {
                let n = name.to_string_lossy();
                if n == "style.css" || n == "config.toml" {
                    match event_type {
                        gio::FileMonitorEvent::Changed | gio::FileMonitorEvent::ChangesDoneHint => {
                            load_custom_css();
                        }
                        _ => {}
                    }
                }
            }
        });
        Box::leak(Box::new(monitor));
    }
}

fn main() {
    // 1. Initialize Adw/Gtk before ANY other logic
    adw::init().expect("Failed to initialize Libadwaita");

    // --- GTK THEME RE-SPAWN HACK ---
    if std::env::var("GTK_THEME").is_err() {
        let settings = gio::Settings::new("org.gnome.desktop.interface");
        let theme_name: String = settings.string("gtk-theme").into();

        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(std::env::args().skip(1))
            .env("GTK_THEME", &theme_name)
            .status()
            .expect("Failed to restart Flux with GTK_THEME");

        std::process::exit(status.code().unwrap_or(0));
    }

    let args: Vec<String> = std::env::args().collect();

    // 2. Now safe to call functions using Gtk objects
    load_custom_css();
    setup_css_watcher();

    // --- CLI HANDLER: FILE PROPERTIES ---
    if args.len() > 2 && args[1] == "--file-properties" {
        let path = PathBuf::from(&args[2]);
        let app = RelmApp::new("flux.PropertiesViewer");
        app.allow_multiple_instances(true);
        app.with_args(vec![]).run::<FileProperties>(path);
        return;
    }

    // --- MAIN APP HANDLER ---
    let app = RelmApp::new("flux.FileManager");
    app.allow_multiple_instances(true);

    let display = adw::gdk::Display::default().expect("Could not get default display");
    let _theme = gtk::IconTheme::for_display(&display);

    let start_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        dirs::home_dir().unwrap_or(PathBuf::from("."))
    };

    app.with_args(vec![]).run::<FluxApp>(start_path);
}
