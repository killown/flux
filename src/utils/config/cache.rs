use parking_lot::RwLock;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::defaults::load_config_from_disk;
use super::snapshots::{extract_icon_config, extract_thumb_config, ICON_CONFIG, THUMB_CONFIG};

static CONFIG_CACHE: OnceLock<RwLock<Option<crate::model::Config>>> = OnceLock::new();

#[inline]
fn config_cache() -> &'static RwLock<Option<crate::model::Config>> {
    CONFIG_CACHE.get_or_init(|| RwLock::new(None))
}

pub fn save_config(config: &crate::model::Config) {
    if let Some(lock) = THUMB_CONFIG.get() {
        *lock.write() = extract_thumb_config(config);
    }
    if let Some(lock) = ICON_CONFIG.get() {
        *lock.write() = extract_icon_config(config);
    }
    {
        let mut cache = config_cache().write();
        *cache = Some(config.clone());
    }

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    let config_path = config_dir.join("config.toml");

    if let Ok(toml_str) = toml::to_string_pretty(config) {
        let tmp_path = config_dir.join(".config.toml.tmp");
        if fs::write(&tmp_path, toml_str).is_ok() {
            let _ = fs::rename(&tmp_path, &config_path);
        }
    }
}

pub fn invalidate_config_cache() {
    if let Some(lock) = CONFIG_CACHE.get() {
        *lock.write() = None;
    }
}

pub fn load_config() -> crate::model::Config {
    crate::hit!("load_config");
    {
        let cache = config_cache().read();
        if let Some(cfg) = cache.as_ref() {
            return cfg.clone();
        }
    }
    let config = load_config_from_disk();
    let mut cache = config_cache().write();
    if cache.is_none() {
        *cache = Some(config.clone());
    }
    cache.as_ref().unwrap().clone()
}
