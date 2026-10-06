//! Theme CSS loading, user overrides, UI scale and theme discovery.

use gtk::glib;
use relm4::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

thread_local! {
    static ACTIVE_CSS_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static ACTIVE_USER_CSS_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Replaces the provider kept in `slot` with a new one built from `css`.
fn swap_css_provider(
    slot: &'static std::thread::LocalKey<RefCell<Option<gtk::CssProvider>>>,
    display: &adw::gdk::Display,
    css: Option<&str>,
    priority: u32,
) {
    slot.with(|cell| {
        let mut guard = cell.borrow_mut();

        if let Some(ref old) = *guard {
            gtk::style_context_remove_provider_for_display(display, old);
        }

        let provider = gtk::CssProvider::new();
        provider.load_from_data(css.unwrap_or(""));
        gtk::style_context_add_provider_for_display(display, &provider, priority);

        *guard = Some(provider);
    });
}

/// Applies runtime font DPI scaling to the default GTK settings.
pub fn apply_ui_scale(scale: f64) {
    let clamped = scale.clamp(0.75, 3.0);

    if let Some(settings) = gtk::Settings::default() {
        let dpi = (96.0 * clamped * 1024.0).round() as i32;
        settings.set_gtk_xft_dpi(dpi);
    }
}

/// Applies the theme CSS, then the user's style.css on top as overrides.
pub fn load_custom_css() {
    crate::hit!("load_custom_css");
    let config = crate::utils::load_config();
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");

    let mut theme_css = None;

    if let Some(ref theme_name) = config.ui.theme {
        if theme_name != "default" {
            let theme_filename = format!("{}.css", theme_name);

            let local_theme = dirs::data_local_dir()
                .unwrap_or_default()
                .join("flux/themes")
                .join(&theme_filename);

            let user_conf_theme = config_dir.join("themes").join(&theme_filename);
            let flatpak_theme = PathBuf::from("/app/share/flux/themes").join(&theme_filename);
            let system_theme = PathBuf::from("/usr/share/flux/themes").join(&theme_filename);

            theme_css = fs::read_to_string(&local_theme)
                .or_else(|_| fs::read_to_string(&user_conf_theme))
                .or_else(|_| fs::read_to_string(&flatpak_theme))
                .or_else(|_| fs::read_to_string(&system_theme))
                .ok();

            if theme_css.is_none() {
                for dir in glib::system_data_dirs() {
                    let candidate = dir.join("flux/themes").join(&theme_filename);
                    if let Ok(content) = fs::read_to_string(&candidate) {
                        theme_css = Some(content);
                        break;
                    }
                }
            }
        }
    }

    // User overrides: ~/.config/flux/style.css loads on top of the theme and always wins.
    let user_css = fs::read_to_string(config_dir.join("style.css"))
        .or_else(|_| fs::read_to_string("/app/share/flux/style.css"))
        .ok();

    if let Some(display) = adw::gdk::Display::default() {
        swap_css_provider(
            &ACTIVE_CSS_PROVIDER,
            &display,
            theme_css.as_deref(),
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
        // One step above the theme so overrides win regardless of load order.
        swap_css_provider(
            &ACTIVE_USER_CSS_PROVIDER,
            &display,
            user_css.as_deref(),
            gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
        );
    }

    if let Some(ref theme_name) = config.ui.theme {
        let style_manager = adw::StyleManager::default();
        if theme_name.contains("dark") {
            style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
        } else if theme_name.contains("light") {
            style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
        } else {
            style_manager.set_color_scheme(adw::ColorScheme::Default);
        }
    }
}

pub fn list_available_themes() -> std::collections::BTreeSet<String> {
    let mut theme_names = std::collections::BTreeSet::new();

    let mut search_dirs = vec![
        dirs::config_dir().unwrap_or_default().join("flux/themes"),
        dirs::data_local_dir()
            .unwrap_or_default()
            .join("flux/themes"),
        std::path::PathBuf::from("/app/share/flux/themes"),
        std::path::PathBuf::from("/usr/share/flux/themes"),
    ];

    for dir in glib::system_data_dirs() {
        search_dirs.push(dir.join("flux/themes"));
    }

    for dir in search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().is_some_and(|ext| ext == "css") {
                    if let Some(n) = path.file_stem().and_then(|n| n.to_str()) {
                        if n != "default" && n != "style" {
                            theme_names.insert(n.to_string());
                        }
                    }
                }
            }
        }
    }

    theme_names
}
