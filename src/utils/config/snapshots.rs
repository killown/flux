use super::cache::load_config;
use crate::model::ThumbnailTypes;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub(super) type ThumbConfigSnapshot = (bool, i32, ThumbnailTypes, usize, bool, f64);

pub(super) static THUMB_CONFIG: OnceLock<RwLock<ThumbConfigSnapshot>> = OnceLock::new();

pub(super) fn extract_thumb_config(config: &crate::model::Config) -> ThumbConfigSnapshot {
    (
        config.ui.show_thumbnails,
        config.ui.thumbnail_size.clamp(16, 768),
        config.ui.thumbnail_types.clone(),
        config.ui.ffmpeg_threads.max(1),
        config.ui.ffmpeg_auto_rotate,
        config.ui.ffmpeg_seek_seconds.max(0.0),
    )
}

pub fn get_thumb_config() -> ThumbConfigSnapshot {
    THUMB_CONFIG
        .get_or_init(|| {
            let config = load_config();
            RwLock::new(extract_thumb_config(&config))
        })
        .read()
        .clone()
}

/// The actual data stored inside the Arc.
pub(super) type IconConfigData = (
    HashMap<String, String>, // folder_icons
    HashMap<String, String>, // file_icons
    bool,                    // auto_generate_mime_icons
    String,                  // auto_mime_accent_color
    String,                  // auto_mime_body_color
    String,                  // auto_mime_font_color
    f64,                     // auto_mime_font_size
);

/// Public so it can be used by other utils modules for lazy caching.
pub type IconConfigSnapshot = Arc<IconConfigData>;

pub(super) static ICON_CONFIG: OnceLock<RwLock<IconConfigSnapshot>> = OnceLock::new();

pub(super) fn extract_icon_config(config: &crate::model::Config) -> IconConfigSnapshot {
    Arc::new((
        config.ui.folder_icons.clone(),
        config.ui.file_icons.clone(),
        config.ui.auto_generate_mime_icons,
        config.ui.auto_mime_accent_color.clone(),
        config.ui.auto_mime_body_color.clone(),
        config.ui.auto_mime_font_color.clone(),
        config.ui.auto_mime_font_size,
    ))
}

pub fn get_icon_config() -> IconConfigSnapshot {
    ICON_CONFIG
        .get_or_init(|| {
            let config = load_config();
            RwLock::new(extract_icon_config(&config))
        })
        .read()
        .clone()
}
