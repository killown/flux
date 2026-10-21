use std::fs;
use std::path::{Path, PathBuf};

/// FreeDesktop thumbnail size tiers, smallest first, with their nominal pixel size.
const TIERS: [(&str, i32); 4] = [
    ("normal", 128),
    ("large", 256),
    ("x-large", 512),
    ("xx-large", 1024),
];

/// Returns the thumbnail tier folder that stores thumbnails generated for `target_size`.
pub fn tier_for_size(target_size: i32) -> &'static str {
    match target_size {
        s if s > 512 => "xx-large",
        s if s > 256 => "x-large",
        s if s > 128 => "large",
        _ => "normal",
    }
}

/// Returns the tiers that may satisfy `target_size`: its own tier first, then larger ones, never smaller.
pub fn thumbnail_lookup_tiers(target_size: i32) -> Vec<&'static str> {
    let own = tier_for_size(target_size);
    TIERS
        .iter()
        .skip_while(|(name, _)| *name != own)
        .map(|(name, _)| *name)
        .collect()
}

/// Finds an existing cached thumbnail for `hash` under `thumb_root` in an acceptable tier.
pub fn find_cached_thumbnail(
    thumb_root: &Path,
    hash: &str,
    target_size: i32,
) -> Option<(PathBuf, fs::Metadata)> {
    crate::hit!("find_cached_thumbnail");
    thumbnail_lookup_tiers(target_size)
        .into_iter()
        .find_map(|tier| {
            let candidate = thumb_root.join(tier).join(format!("{hash}.png"));
            let meta = fs::metadata(&candidate).ok()?;
            meta.is_file().then_some((candidate, meta))
        })
}
