use crate::model::{Config, FluxApp};
use std::path::Path;

/// Records a custom icon for `target` in the loaded config.
///
/// Directories also propagate the icon to any matching pinned sidebar entry,
/// so the change is visible there without a separate edit.
pub(super) fn set_icon_on_target(config: &mut Config, target: &Path, image: &Path) {
    let key = target.to_string_lossy().to_string();
    let img_val = image.to_string_lossy().to_string();

    if target.is_dir() {
        config.ui.folder_icons.insert(key, img_val.clone());
        for place in &mut config.sidebar {
            let expanded = FluxApp::expand_path(&place.path);
            if expanded == target {
                place.icon = img_val.clone();
            }
        }
    } else {
        config.ui.file_icons.insert(key, img_val);
    }
}

/// Removes any custom icon for `target` from the loaded config.
pub(super) fn reset_icon_on_target(config: &mut Config, target: &Path) {
    let key = target.to_string_lossy().to_string();
    config.ui.file_icons.remove(&key);
    config.ui.folder_icons.remove(&key);
}
