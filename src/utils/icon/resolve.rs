use super::cache::THEMED_ICON_CACHE;
use super::theme::resolve_folder_icon_with_fallbacks;
use super::xdg::get_default_xdg_folder_icon;
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use std::path::Path;

/// Returns a GIO icon for `path` using the default (no-override) resolution.
pub fn get_icon_for_path(path: &Path, is_dir: bool) -> adw::gio::Icon {
    get_icon_for_path_with_override(path, is_dir, None)
}

#[inline]
fn is_generic_icon_name(name: &str) -> bool {
    name.ends_with("-x-generic") || name == "application-octet-stream" || name == "unknown"
}

/// Returns a GIO icon for the given path, applying a custom icon name override
/// when provided.
///
/// # Arguments
///
/// * `path`        - absolute path to the file or directory.
/// * `is_dir`      - whether the entry is a directory.
/// * `custom_icon` - optional GTK icon name to use instead of the derived default.
pub fn get_icon_for_path_with_override(
    path: &Path,
    is_dir: bool,
    custom_icon: Option<&str>,
) -> adw::gio::Icon {
    crate::hit!("get_icon_for_path");

    // ── Explicit custom override ────────────────────────────────────────────
    if let Some(icon_name) = custom_icon {
        if let Ok(icon) = gio::Icon::for_string(icon_name) {
            return icon;
        }
    }

    let mut icon_config_cache: Option<crate::utils::config::IconConfigSnapshot> = None;
    let mut get_icon_config_lazy = || -> crate::utils::config::IconConfigSnapshot {
        icon_config_cache
            .get_or_insert_with(crate::utils::config::get_icon_config)
            .clone()
    };

    let mut canon_str_cache: Option<Option<String>> = None;
    let mut get_canon_str_lazy = || -> Option<String> {
        canon_str_cache
            .get_or_insert_with(|| {
                path.canonicalize()
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .clone()
    };

    let path_str = path.to_string_lossy();

    // ── Directories ─────────────────────────────────────────────────────────
    if is_dir {
        let (folder_icons, _, _, _, _, _, _) = &*get_icon_config_lazy();

        let mut folder_match = folder_icons.get(path_str.as_ref());
        if folder_match.is_none() {
            if let Some(ref canon) = get_canon_str_lazy() {
                folder_match = folder_icons.get(canon);
            }
        }

        if let Some(custom) = folder_match {
            if let Ok(icon) = gio::Icon::for_string(custom) {
                return icon;
            }
        }

        if let Some(base_icon) = get_default_xdg_folder_icon(path) {
            if let Some(display) = gdk::Display::default() {
                let theme = gtk::IconTheme::for_display(&display);
                if let Some(icon) = resolve_folder_icon_with_fallbacks(&theme, base_icon) {
                    return icon;
                }
            } else if let Ok(icon) = gio::Icon::for_string(base_icon) {
                return icon;
            }
        }

        return gio::Icon::for_string("folder").unwrap();
    }

    // ── Files: per-path custom icon ─────────────────────────────────────────
    let (_, file_icons, _, _, _, _, _) = &*get_icon_config_lazy();

    let mut file_match = file_icons.get(path_str.as_ref());
    if file_match.is_none() {
        if let Some(ref canon) = get_canon_str_lazy() {
            file_match = file_icons.get(canon);
        }
    }

    if let Some(custom) = file_match {
        if let Ok(icon) = gio::Icon::for_string(custom) {
            return icon;
        }
    }

    // ── Filename / extension (computed once) ────────────────────────────────
    let filename = if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
        path_str.rsplit('/').next().unwrap_or("").to_string()
    } else {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };

    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // `.desktop` file: read the embedded Icon= key.
    if ext.eq_ignore_ascii_case("desktop") {
        let keyfile = glib::KeyFile::new();
        if keyfile
            .load_from_file(path, glib::KeyFileFlags::NONE)
            .is_ok()
        {
            if let Ok(icon_val) = keyfile.string(
                glib::KEY_FILE_DESKTOP_GROUP,
                glib::KEY_FILE_DESKTOP_KEY_ICON,
            ) {
                if !icon_val.is_empty() {
                    if let Ok(icon) = gio::Icon::for_string(icon_val.as_str()) {
                        return icon;
                    }
                }
            }
        }
    }

    // Per-extension custom icon (only place this lookup happens now).
    if !ext.is_empty() {
        if let Some(icon_path) = crate::services::loader::get_custom_extension_icon_path(ext) {
            if let Ok(icon) = gio::Icon::for_string(&icon_path.to_string_lossy()) {
                return icon;
            }
        }
    }

    // ── MIME resolution ─────────────────────────────────────────────────────
    let content_type = if let Some(mime) = crate::utils::media::guess_mime_from_extension(&filename)
    {
        mime
    } else if crate::services::network::is_network_uri(path)
        || path_str.starts_with(crate::services::archive::ARCHIVE_URI)
    {
        "application/octet-stream".to_string()
    } else {
        let (ct, _) = adw::gio::content_type_guess(Some(filename.as_str()), None::<&[u8]>);
        ct.to_string()
    };

    // ── Cache-aware lookup ──────────────────────────────────────────────────
    //
    // WARNING: keep the cache two-tiered. Do not serve `content_type` entries
    // to extensioned lookups, and do not merge the tiers into one map.
    // Extensionless files plant a shared fallback that would then
    // short-circuit every .txt / .conf / .log past generation, killing
    // generated icons for the rest of the session.
    THEMED_ICON_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();

        let gen_key = if ext.is_empty() {
            String::new()
        } else {
            format!("ext:{}", ext)
        };

        if !gen_key.is_empty() {
            if let Some(icon) = map.get(&gen_key) {
                return icon.clone();
            }
        }

        if gen_key.is_empty() {
            if let Some(icon) = map.get(&content_type) {
                return icon.clone();
            }
        }

        let icon = adw::gio::content_type_get_icon(&content_type);

        // WARNING: do not drop either condition. Without the theme check,
        // .png / .mp4 / .pdf ignore purpose-drawn theme icons. Without
        // `container_mime_masks_extension`, .conf / .cfg / .dat inherit the
        // application/xml icon instead of generating.
        let has_specific_theme_icon = if let Some(display) = gdk::Display::default() {
            let theme = gtk::IconTheme::for_display(&display);
            if let Some(themed) = icon.downcast_ref::<gio::ThemedIcon>() {
                let theme_hit = themed
                    .names()
                    .iter()
                    .any(|name| !is_generic_icon_name(name.as_str()) && theme.has_icon(name));

                theme_hit
                    && !crate::utils::media::container_mime_masks_extension(ext, &content_type)
            } else {
                false
            }
        } else {
            false
        };

        if has_specific_theme_icon {
            if !gen_key.is_empty() {
                map.insert(gen_key, icon.clone());
            } else {
                map.insert(content_type, icon.clone());
            }
            return icon;
        }

        // Theme has no dedicated icon: generate ONLY if <= 9 chars.
        if !ext.is_empty() && ext.len() <= 9 {
            let (_, _, auto_gen, _, _, _, _) = &*get_icon_config_lazy();
            if *auto_gen {
                if let Some(generated_path) = crate::services::loader::get_extension_icon_path(ext)
                {
                    if let Ok(generated_icon) =
                        gio::Icon::for_string(&generated_path.to_string_lossy())
                    {
                        map.insert(gen_key, generated_icon.clone());
                        return generated_icon;
                    }
                }
            }
        }

        // Generic theme fallback when no dedicated or generated icon matched.
        // NOTE: Cache under `gen_key` for extensioned files to avoid poisoning
        // the shared `content_type` slot for other extensions.
        if !gen_key.is_empty() {
            map.insert(gen_key, icon.clone());
        } else {
            map.insert(content_type, icon.clone());
        }
        icon
    })
}
