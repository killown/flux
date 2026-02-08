mod app;
mod ui_components;
mod utils;
mod model;
mod file_properties;

use relm4::prelude::*;
use crate::model::FluxApp;
use crate::file_properties::FileProperties;
use std::path::PathBuf;
use adw::prelude::*;
use adw::gio;
use std::fs;

// Thread-local provider allows load_from_data to overwrite previous styles on the same object
thread_local! {
    static CSS_PROVIDER: gtk::CssProvider = gtk::CssProvider::new();
}

/// Loads CSS and reports source. Uses higher priority for external files to replace !important.
fn load_custom_css() {
    let css_path = dirs::config_dir().unwrap_or_default().join("flux/style.css");

    let (css_data, source, priority) = match fs::read_to_string(&css_path) {
        Ok(data) => (
            data, 
            css_path.to_string_lossy().to_string(),
            gtk::STYLE_PROVIDER_PRIORITY_USER
        ),
        Err(_) => (
            include_str!("style.css").to_string(),
            "internal fallback".to_string(),
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION
        ),
    };

    println!("[flux] CSS LOADING: {}", source);

    CSS_PROVIDER.with(|provider| {
        provider.load_from_data(&css_data);

        if let Some(display) = adw::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                provider,
                priority,
            );
        }
    });
}

/// Sets up a GIO file monitor for live CSS reloading
fn setup_css_watcher() {
    let css_path = dirs::config_dir().unwrap_or_default().join("flux/style.css");
    let file = gio::File::for_path(&css_path);

    if let Ok(monitor) = file.monitor(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        monitor.connect_changed(|_, _, _, event_type| {
            match event_type {
                gio::FileMonitorEvent::Changed | gio::FileMonitorEvent::ChangesDoneHint => {
                    load_custom_css();
                }
                _ => {}
            }
        });
        // Leak the monitor to keep the subscription active for the life of the process
        Box::leak(Box::new(monitor));
    }
}

fn main() {
    // --- GTK THEME RE-SPAWN HACK ---
    // If GTK_THEME is not set, the app won't inherit the system-level Adwaita styling correctly.
    if std::env::var("GTK_THEME").is_err() {
        // 1. Get the current theme from GSettings
        let settings = gio::Settings::new("org.gnome.desktop.interface");
        let theme_name: String = settings.string("gtk-theme").into();

        // 2. Re-spawn the process with the variable set
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(std::env::args().skip(1))
            .env("GTK_THEME", &theme_name)
            .status()
            .expect("Failed to restart Flux with GTK_THEME");

        // 3. Exit the initial "themeless" process
        std::process::exit(status.code().unwrap_or(0));
    }

    let args: Vec<String> = std::env::args().collect();

    // --- CLI HANDLER: FILE PROPERTIES ---
    if args.len() > 2 && args[1] == "--file-properties" {
        let path = PathBuf::from(&args[2]);
        // Separate RelmApp instances to satisfy different Component::Input types
        let app = RelmApp::new("flux.PropertiesViewer");
        app.allow_multiple_instances(true);

        load_custom_css();
        setup_css_watcher();

        app.with_args(vec![])
           .run::<FileProperties>(path);
        return;
    }

    // --- MAIN APP HANDLER ---
    let app = RelmApp::new("flux.FileManager");
    app.allow_multiple_instances(true);

    load_custom_css();
    setup_css_watcher();

    let display = adw::gdk::Display::default().expect("Could not get default display");
    let _theme = gtk::IconTheme::for_display(&display);

    let start_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        dirs::home_dir().unwrap_or(PathBuf::from("."))
    };

    app.with_args(vec![])
       .run::<FluxApp>(start_path);
}
