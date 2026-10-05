use crate::model::FileLoadContext;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::SystemTime;

static CUSTOM_EXT_CACHE: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
static GENERATED_EXT_CACHE: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
static RESOLVED_EXT_CACHE: OnceLock<RwLock<HashMap<String, Option<PathBuf>>>> = OnceLock::new();

fn scan_directory_extensions(subpath: &str) -> HashSet<String> {
    crate::hit!("scan_directory_extensions");
    let mut set = HashSet::new();
    if let Some(icons_dir) = dirs::data_local_dir().map(|d| d.join(subpath)) {
        if let Ok(entries) = std::fs::read_dir(&icons_dir) {
            for entry in entries.flatten() {
                if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                    set.insert(stem.to_ascii_lowercase());
                }
            }
        }
    }
    set
}

fn scan_custom_extension_icons() -> HashSet<String> {
    scan_directory_extensions("flux/icons/extensions/custom")
}

fn scan_generated_extension_icons() -> HashSet<String> {
    scan_directory_extensions("flux/icons/extensions/generated")
}

/// Returns the newest modification time between template.svg and config.toml.
fn get_template_and_config_mtime() -> Option<SystemTime> {
    let template_mtime = dirs::data_dir()
        .map(|d| d.join("flux/icons/template.svg"))
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());

    let config_mtime = dirs::config_dir()
        .map(|d| d.join("flux/config.toml"))
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());

    match (template_mtime, config_mtime) {
        (Some(t1), Some(t2)) => Some(t1.max(t2)),
        (Some(t), None) | (None, Some(t)) => Some(t),
        (None, None) => None,
    }
}

/// Returns the path to a user-customized extension icon from `icons/extensions/custom/`.
pub fn get_custom_extension_icon_path(ext: &str) -> Option<PathBuf> {
    if ext.is_empty() {
        return None;
    }
    let ext_lower = ext.to_ascii_lowercase();
    let cache_lock = CUSTOM_EXT_CACHE.get_or_init(|| RwLock::new(scan_custom_extension_icons()));
    if !cache_lock.read().contains(&ext_lower) {
        return None;
    }

    let custom_dir = dirs::data_local_dir()?.join("flux/icons/extensions/custom");
    for format in &["png", "svg", "webp", "jpg", "jpeg"] {
        let candidate = custom_dir.join(format!("{}.{}", ext_lower, format));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Returns the path to an auto-generated extension icon from `icons/extensions/generated/`.
pub fn get_generated_extension_icon_path(ext: &str) -> Option<PathBuf> {
    if ext.is_empty() {
        return None;
    }
    let ext_lower = ext.to_ascii_lowercase();
    let cache_lock =
        GENERATED_EXT_CACHE.get_or_init(|| RwLock::new(scan_generated_extension_icons()));

    let exists = cache_lock.read().contains(&ext_lower);
    let gen_dir = dirs::data_local_dir()?.join("flux/icons/extensions/generated");
    let latest_source_mtime = get_template_and_config_mtime();

    if exists {
        for format in &["png", "svg", "webp", "jpg", "jpeg"] {
            let candidate = gen_dir.join(format!("{}.{}", ext_lower, format));
            if candidate.exists() {
                // If it's an SVG and template/config was modified AFTER this icon was generated, re-generate it
                if *format == "svg" {
                    if let (Some(source_mt), Ok(meta)) =
                        (latest_source_mtime, std::fs::metadata(&candidate))
                    {
                        if let Ok(icon_mt) = meta.modified() {
                            if source_mt > icon_mt {
                                let (auto_gen, accent, body, font, font_size) =
                                    crate::utils::media::icon_gen_params();
                                if auto_gen {
                                    if let Ok(rebuilt) =
            crate::utils::extension_template::save_generated_extension_icon(
                &ext_lower, &accent, &body, &font, font_size,
            )
        {
            return Some(rebuilt);
        }
                                }
                            }
                        }
                    }
                }
                return Some(candidate);
            }
        }
    }

    None
}

/// Returns the effective icon path for an extension, prioritizing `custom/` over `generated/`,
/// and generating into `generated/` on demand if absent.
pub fn get_extension_icon_path(ext: &str) -> Option<PathBuf> {
    if ext.is_empty() {
        return None;
    }
    let ext_lower = ext.to_ascii_lowercase();

    let resolved_map = RESOLVED_EXT_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read_guard = resolved_map.read();
        if let Some(cached_result) = read_guard.get(&ext_lower) {
            return cached_result.clone();
        }
    }

    if let Some(custom) = get_custom_extension_icon_path(&ext_lower) {
        resolved_map.write().insert(ext_lower, Some(custom.clone()));
        return Some(custom);
    }

    if let Some(gen) = get_generated_extension_icon_path(&ext_lower) {
        resolved_map.write().insert(ext_lower, Some(gen.clone()));
        return Some(gen);
    }

    let (auto_gen, accent, body, font, font_size) = crate::utils::media::icon_gen_params();

    let result = if auto_gen {
        if let Ok(generated) = crate::utils::extension_template::save_generated_extension_icon(
            &ext_lower, &accent, &body, &font, font_size,
        ) {
            let gen_lock =
                GENERATED_EXT_CACHE.get_or_init(|| RwLock::new(scan_generated_extension_icons()));
            gen_lock.write().insert(ext_lower.clone());
            Some(generated)
        } else {
            None
        }
    } else {
        None
    };

    resolved_map.write().insert(ext_lower, result.clone());
    result
}

/// Invalidates all extension icon lookup caches.
pub fn invalidate_extension_icon_cache() {
    let custom_lock = CUSTOM_EXT_CACHE.get_or_init(|| RwLock::new(scan_custom_extension_icons()));
    *custom_lock.write() = scan_custom_extension_icons();

    let gen_lock =
        GENERATED_EXT_CACHE.get_or_init(|| RwLock::new(scan_generated_extension_icons()));
    *gen_lock.write() = scan_generated_extension_icons();

    let resolved_map = RESOLVED_EXT_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    resolved_map.write().clear();
}

/// Returns the path to use as thumbnail source, giving priority to custom icon.
#[allow(dead_code)]
pub fn resolve_thumb_source(ctx: &FileLoadContext) -> Option<PathBuf> {
    ctx.custom_icon
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| ctx.thumbnail_path.clone())
}
