use gtk::prelude::*;
use std::path::PathBuf;

/// Walk upward from `start` looking for a widget carrying a `flux_path` data entry.
pub fn find_flux_path_ancestor(start: gtk::Widget) -> Option<PathBuf> {
    let mut current: Option<gtk::Widget> = Some(start);
    while let Some(w) = current {
        let path: Option<PathBuf> =
            unsafe { w.data::<PathBuf>("flux_path").map(|p| p.as_ref().clone()) };
        if path.is_some() {
            return path;
        }
        current = w.parent();
    }
    None
}
