//! Custom background image slots injected as CSS.

use adw::prelude::*;
use gtk::gdk;
use relm4::prelude::*;

thread_local! {
    static ACTIVE_BG_CSS_PROVIDER: std::cell::RefCell<Option<gtk::CssProvider>> = const { std::cell::RefCell::new(None) };
}

/// Finds an image in `base_dir` matching the slot prefix (e.g. `window.png` or `window_<timestamp>.png`).
fn find_slot_image(base_dir: &std::path::Path, prefix: &str) -> Option<std::path::PathBuf> {
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if stem == prefix || stem.starts_with(&format!("{}_", prefix)) {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Injects dynamic background image styles if the image files exist on disk.
pub fn load_custom_background_images() {
    let Some(data_dir) = dirs::data_dir() else {
        return;
    };
    let base_img_dir = data_dir.join("flux/data/resources/images");

    let sidebar_left = find_slot_image(&base_img_dir, "sidebar-left");
    let sidebar_right = find_slot_image(&base_img_dir, "sidebar-right");
    let window_bg = find_slot_image(&base_img_dir, "window");

    let config = crate::utils::load_config();
    let alpha_left =
        format!("{:.2}", config.ui.bg_alpha_sidebar_left.clamp(0.0, 1.0)).replace(',', ".");
    let alpha_window =
        format!("{:.2}", config.ui.bg_alpha_window.clamp(0.0, 1.0)).replace(',', ".");
    let alpha_right =
        format!("{:.2}", config.ui.bg_alpha_sidebar_right.clamp(0.0, 1.0)).replace(',', ".");

    let mut css = String::new();

    if let Some(ref path) = sidebar_left {
        css.push_str(&format!(
            r#".sidebar {{
  background-image:
    linear-gradient(rgba(30, 30, 30, {alpha}), rgba(30, 30, 30, {alpha})),
    url("file://{path}");
  background-repeat: no-repeat;
  background-position: center;
  background-size: cover;
  background-clip: padding-box;
}}
"#,
            alpha = alpha_left,
            path = path.to_string_lossy(),
        ));
    }

    if let Some(ref path) = window_bg {
        css.push_str(&format!(
            r#"window,
window.background {{
  background-image:
    linear-gradient(rgba(30, 30, 30, {alpha}), rgba(30, 30, 30, {alpha})),
    url("file://{path}");
  background-repeat: no-repeat;
  background-position: center;
  background-size: cover;
  background-clip: padding-box;
}}

/* Prevent the wallpaper from painting into the window frame border and shadow */
window.solid-csd,
window.csd {{
  background-clip: padding-box;
  border-color: rgba(30, 30, 30, 0.8);
}}
"#,
            alpha = alpha_window,
            path = path.to_string_lossy(),
        ));
    }

    if let Some(ref path) = sidebar_right {
        css.push_str(&format!(
            r#"revealer > box > .sidebar {{
  background-image:
    linear-gradient(rgba(30, 30, 30, {alpha}), rgba(30, 30, 30, {alpha})),
    url("file://{path}");
  background-repeat: no-repeat;
  background-position: center;
  background-size: cover;
  background-clip: padding-box;
}}
"#,
            alpha = alpha_right,
            path = path.to_string_lossy(),
        ));
    }

    if let Some(display) = gdk::Display::default() {
        ACTIVE_BG_CSS_PROVIDER.with(|cell| {
            let mut guard = cell.borrow_mut();
            if let Some(ref old_provider) = *guard {
                gtk::style_context_remove_provider_for_display(&display, old_provider);
            }

            if !css.is_empty() {
                let provider = gtk::CssProvider::new();
                provider.load_from_data(&css);
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
                *guard = Some(provider);
            } else {
                *guard = None;
            }
        });

        // Trigger immediate style recalculation across active windows
        let toplevels = gtk::Window::toplevels();
        for i in 0..toplevels.n_items() {
            if let Some(window) = toplevels.item(i).and_downcast::<gtk::Window>() {
                window.add_css_class("flux-bg-sync");
                window.remove_css_class("flux-bg-sync");
                window.queue_draw();
            }
        }
    }
}

/// Deletes all background images from disk and clears the CSS provider.
pub fn clear_custom_background_images() {
    if let Some(data_dir) = dirs::data_dir() {
        let base_img_dir = data_dir.join("flux/data/resources/images");
        if let Ok(entries) = std::fs::read_dir(&base_img_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("sidebar-left")
                        || name.starts_with("sidebar-right")
                        || name.starts_with("window")
                    {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    if let Some(display) = gdk::Display::default() {
        ACTIVE_BG_CSS_PROVIDER.with(|cell| {
            let mut guard = cell.borrow_mut();
            if let Some(ref old_provider) = guard.take() {
                gtk::style_context_remove_provider_for_display(&display, old_provider);
            }
        });

        let toplevels = gtk::Window::toplevels();
        for i in 0..toplevels.n_items() {
            if let Some(window) = toplevels.item(i).and_downcast::<gtk::Window>() {
                window.add_css_class("flux-bg-sync");
                window.remove_css_class("flux-bg-sync");
                window.queue_draw();
            }
        }
    }
}
