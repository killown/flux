use std::path::PathBuf;

use gtk::gio;
use gtk::prelude::*;

use super::debug::net_debug;
use super::protocol::NetworkProtocol;
use crate::model::FileLoadContext;

#[derive(Debug, Clone)]
pub struct NetworkEntry {
    pub display_name: String,
    pub uri: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
    pub icon_name: Option<String>,
    pub protocol: Option<NetworkProtocol>,
}

/// Reads the first themed icon name off a `GIcon`, or `None` when the icon
/// is not a `GThemedIcon`. Do not use `icon.to_string()` here: for a
/// `GThemedIcon` that yields the debug repr (`". GThemedIcon inode-directory …"`)
/// which is not a valid icon name and forces every caller to fall back to a
/// slow path in `get_icon_for_path`.
#[inline]
pub(super) fn themed_icon_name(icon: Option<gio::Icon>) -> Option<String> {
    let icon = icon?;
    let themed = icon.downcast_ref::<gio::ThemedIcon>()?;
    themed.names().first().map(|s| s.to_string())
}

pub fn entries_to_load_contexts(
    entries: &[NetworkEntry],
    expand_labels: bool,
) -> Vec<FileLoadContext> {
    net_debug!(
        "[network] entries_to_load_contexts: {} entries",
        entries.len()
    );
    entries
        .iter()
        .map(|e| {
            let target_path = PathBuf::from(&e.uri);
            let sort_name = e.display_name.to_lowercase();
            let sort_ext = std::path::Path::new(&e.display_name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            let custom_icon = if e.is_dir {
                e.icon_name
                    .clone()
                    .or_else(|| e.protocol.as_ref().map(|p| p.icon_name().to_owned()))
            } else {
                None
            };

            FileLoadContext::with_stats(
                e.display_name.clone(),
                target_path,
                e.is_dir,
                sort_name,
                sort_ext,
                e.size,
                e.mtime,
                None,
                expand_labels,
                custom_icon,
            )
        })
        .collect()
}
