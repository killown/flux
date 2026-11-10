use parking_lot::{Mutex, RwLock};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use super::defaults::load_config_from_disk;
use super::snapshots::{extract_icon_config, extract_thumb_config, ICON_CONFIG, THUMB_CONFIG};

static CONFIG_CACHE: OnceLock<RwLock<Option<Arc<crate::domain::model::Config>>>> = OnceLock::new();

/// Serializes `save_config`, so the cache and `config.toml` receive writes in the same order.
static SAVE_LOCK: Mutex<()> = Mutex::new(());

/// Set while the last write to `config.toml` failed, so an identical config is saved
/// again instead of being skipped as already cached.
static UNSAVED: AtomicBool = AtomicBool::new(false);

/// Bumped on every cache invalidation, letting `load_config` detect that its disk
/// read may predate a change to `config.toml`.
static INVALIDATIONS: AtomicU64 = AtomicU64::new(0);

/// How long the config watcher ignores `config.toml` events after a quiet save.
const WATCHER_MUTE_WINDOW: Duration = Duration::from_millis(1500);

/// Time base of [`WATCHER_MUTED_UNTIL_MS`].
static CLOCK_ORIGIN: OnceLock<Instant> = OnceLock::new();

/// End of the window in which the config watcher ignores `config.toml` events,
/// in milliseconds since [`CLOCK_ORIGIN`]. `0` means no window was ever opened.
static WATCHER_MUTED_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

fn elapsed_ms() -> u64 {
    CLOCK_ORIGIN.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Opens a window in which [`watcher_echo_muted`] returns `true`.
fn mute_watcher() {
    let until = elapsed_ms() + WATCHER_MUTE_WINDOW.as_millis() as u64 + 1;
    WATCHER_MUTED_UNTIL_MS.store(until, Ordering::Relaxed);
}

/// True while the config watcher should ignore `config.toml` events because
/// this process caused them with [`save_config_quiet`].
///
/// The cache already holds the saved values, so the reload the watcher would
/// start (CSS, sidebar, a folder rescan) would only repeat work.
pub fn watcher_echo_muted() -> bool {
    let until = WATCHER_MUTED_UNTIL_MS.load(Ordering::Relaxed);
    until != 0 && elapsed_ms() < until
}

#[inline]
fn config_cache() -> &'static RwLock<Option<Arc<crate::domain::model::Config>>> {
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

pub fn save_config(config: &crate::domain::model::Config) {
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

    let config_dir = crate::utils::config::base_config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    let config_path = config_dir.join("config.toml");

    let _ = fs::create_dir_all(&config_dir);

    let saved = toml::to_string_pretty(config)
        .ok()
        .is_some_and(|toml_str| write_atomic(&config_path, &toml_str).is_ok());
    UNSAVED.store(!saved, Ordering::Relaxed);
}

/// Saves like [`save_config`], and tells this process's config watcher to
/// ignore the file event the write causes.
///
/// For changes that do not need the reload the watcher starts, such as the view
/// mode: a rescan of the open folder after every layout toggle. The window is
/// opened before the write and again after it, because the event arrives when the
/// write ends and the watcher checks 50 ms later. Blocks on disk I/O, so call it
/// from the blocking pool, not from the GTK thread.
pub fn save_config_quiet(config: &crate::domain::model::Config) {
    mute_watcher();
    save_config(config);
    mute_watcher();
}

pub fn invalidate_config_cache() {
    let mut cache = config_cache().write();
    INVALIDATIONS.fetch_add(1, Ordering::SeqCst);
    *cache = None;
}

/// Returns the cached config as a shared handle.
pub fn config_snapshot() -> Arc<crate::domain::model::Config> {
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
pub fn load_config() -> crate::domain::model::Config {
    crate::hit!("load_config");
    (*config_snapshot()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_is_muted_only_inside_the_window() {
        WATCHER_MUTED_UNTIL_MS.store(0, Ordering::Relaxed);
        assert!(!watcher_echo_muted(), "no window was opened yet");

        mute_watcher();
        assert!(
            watcher_echo_muted(),
            "the window is open right after a save"
        );

        // A window that ended before now must not mute anything.
        let now = elapsed_ms();
        std::thread::sleep(Duration::from_millis(5));
        WATCHER_MUTED_UNTIL_MS.store(now + 1, Ordering::Relaxed);
        assert!(!watcher_echo_muted(), "the window has closed");
    }
}
