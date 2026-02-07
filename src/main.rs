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

fn main() {
    // 1. Check if we've already set the theme
    if std::env::var("GTK_THEME").is_err() {
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
    // Usage: flux --file-properties /path/to/file
    if args.len() > 2 && args[1] == "--file-properties" {
        let path = PathBuf::from(&args[2]);
        let app = RelmApp::new("flux.PropertiesViewer");
        app.allow_multiple_instances(true);

        app.with_args(vec![])
           .run::<FileProperties>(path);
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

    app.with_args(vec![])
       .run::<FluxApp>(start_path);
}
