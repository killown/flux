use parking_lot::{Mutex, RwLock};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use super::defaults::load_config_from_disk;
use super::snapshots::{extract_icon_config, extract_thumb_config, ICON_CONFIG, THUMB_CONFIG};

static CONFIG_CACHE: OnceLock<RwLock<Option<Arc<crate::model::Config>>>> = OnceLock::new();

/// Serializes `save_config`, so the cache and `config.toml` receive writes in the same order.
static SAVE_LOCK: Mutex<()> = Mutex::new(());

/// Set while the last write to `config.toml` failed, so an identical config is saved
/// again instead of being skipped as already cached.
static UNSAVED: AtomicBool = AtomicBool::new(false);

/// Bumped on every cache invalidation, letting `load_config` detect that its disk
/// read may predate a change to `config.toml`.
static INVALIDATIONS: AtomicU64 = AtomicU64::new(0);

#[inline]
fn config_cache() -> &'static RwLock<Option<Arc<crate::model::Config>>> {
    CONFIG_CACHE.get_or_init(|| RwLock::new(None))
}

fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let tmp = path.with_file_name(format!(".config.toml.{}.tmp", std::process::id()));
    let result = fs::write(&tmp, contents).and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn save_config(config: &crate::model::Config) {
    crate::hit!("save_config");
    // Skip the whole write when the incoming config is byte-identical to the
    // one already cached. Every settings toggle, watcher event and handler
    // funnels through here, and most of them fire on every keystroke or scroll
    // tick. Writing `config.toml` on each of those would bump its mtime and
    // wake every downstream watcher (config file monitor, fish shell respawn)
    // for no reason.
    //
    // `Config` derives `PartialEq`, so this is a shallow structural compare.
    // It is cheap relative to `toml::to_string_pretty` + atomic rename.
    {
        let cache = config_cache().read();
        if !UNSAVED.load(Ordering::Relaxed) && cache.as_deref() == Some(config) {
            return;
        }
    }

    let _save = SAVE_LOCK.lock();

    if let Some(lock) = THUMB_CONFIG.get() {
        *lock.write() = extract_thumb_config(config);
    }
    if let Some(lock) = ICON_CONFIG.get() {
        *lock.write() = extract_icon_config(config);
    }
    {
        let mut cache = config_cache().write();
        *cache = Some(Arc::new(config.clone()));
    }

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    let config_path = config_dir.join("config.toml");

    let _ = fs::create_dir_all(&config_dir);

    let saved = toml::to_string_pretty(config)
        .ok()
        .is_some_and(|toml_str| write_atomic(&config_path, &toml_str).is_ok());
    UNSAVED.store(!saved, Ordering::Relaxed);
}

pub fn invalidate_config_cache() {
    let mut cache = config_cache().write();
    INVALIDATIONS.fetch_add(1, Ordering::SeqCst);
    *cache = None;
}

/// Returns the cached config as a shared handle.
pub fn config_snapshot() -> Arc<crate::model::Config> {
    for _ in 0..3 {
        {
            let cache = config_cache().read();
            if let Some(cfg) = cache.as_ref() {
                return Arc::clone(cfg);
            }
        }
        let seen = INVALIDATIONS.load(Ordering::SeqCst);
        let config = Arc::new(load_config_from_disk());
        let mut cache = config_cache().write();
        if let Some(cfg) = cache.as_ref() {
            return Arc::clone(cfg);
        }
        if INVALIDATIONS.load(Ordering::SeqCst) == seen {
            *cache = Some(Arc::clone(&config));
            return config;
        }
    }
    let config = Arc::new(load_config_from_disk());
    let mut cache = config_cache().write();
    Arc::clone(cache.get_or_insert(config))
}

/// Returns an owned copy of the config.
pub fn load_config() -> crate::model::Config {
    crate::hit!("load_config");
    (*config_snapshot()).clone()
}
