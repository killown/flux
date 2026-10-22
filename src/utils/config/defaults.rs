use crate::i18n::tr;
use crate::model::{TerminalConfig, TerminalPosition};
use std::fs;
use std::path::PathBuf;

use super::cache::{invalidate_config_cache, save_config};

/// Reads the legacy `ui.terminal_in_sidebar` bool out of the raw config text.
///
/// Deliberately parses the raw TOML rather than a `UIConfig` field: the key is
/// written only by pre-0.6 Flux and must never be serialized again. Returns `None`
/// when the key is absent, not a bool, or the text is not valid TOML.
fn legacy_terminal_in_sidebar(raw: &str) -> Option<bool> {
    toml::from_str::<toml::Value>(raw)
        .ok()?
        .get("ui")?
        .get("terminal_in_sidebar")?
        .as_bool()
}

/// One-shot: migrates the legacy `ui.terminal_in_sidebar` bool into
/// `ui.terminal.position`. `true` becomes `Sidebar` unless the user has already
/// picked a position; `false` leaves the default `Bottom`.
///
/// Returns `true` when the key was present, i.e. `config.toml` must be rewritten.
/// The rewrite drops the key, because `Config` no longer has a field for it.
fn migrate_legacy_terminal_position(config: &mut crate::model::Config, raw: &str) -> bool {
    let Some(legacy) = legacy_terminal_in_sidebar(raw) else {
        return false;
    };
    if legacy && config.ui.terminal.position == TerminalPosition::Bottom {
        config.ui.terminal.position = TerminalPosition::Sidebar;
    }
    true
}

pub(super) fn load_config_from_disk() -> crate::model::Config {
    crate::hit!("load_config_from_disk");
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        let _ = fs::create_dir_all(&config_dir);

        let mut default_toml = String::from(
            r#"[ui]
diff_editor = "nvim"
max_content_search_results = 100
default_icon_size = 96
startup_window_width = 1280
startup_window_height = 800
sidebar_width = 200
single_click = false
show_xdg_dirs = false
default_sort = "Name"
show_hidden_by_default = false
folders_first = true
theme = "default"
show_csd = true
window_controls_left = true
start_maximized = true
show_thumbnails = true

[ui.terminal]
height = 50
fg_color = ""
bg_color = ""
font = "JetBrains Mono 11"
position = "bottom"

[ui.thumbnail_types]
images = true
videos = true
fonts = true
pdfs = true

[ui.folder_sort]

[ui.device_renames]
"/path/to/device/" = { name = "Storage", icon = "drive-harddisk-solid-symbolic" }

[ui.folder_icon_size]

[shortcuts]
# -- Navigation --

# Returns to the previous folder in the linear history stack.
back = "BackSpace"

# Moves forward in the history stack.
forward = "<Alt>Right"

# Activates the selected file or enters the selected directory.
open = "Return"

# -- File Operations --

# Moves selected items to the trash.
delete = "Delete"

# -- View & Application --

# Triggers a reload of the current directory.
refresh = "F5"

# Toggles folder grouping placement (first vs last) in the current directory.
toggle_folders_first = "F7"

# Focuses the search/filter entry bar.
search = "<Primary>f"

# Toggles the visibility of hidden files (dotfiles).
toggle_hidden = "<Primary>h"

[[sidebar]]
name = "Default"
kind = "label"
icon = ""
path = ""

[[sidebar]]
name = "Flux"
icon = "folder-documents-symbolic"
path = "~/.config/flux"


[[sidebar]]
name = "Shortcuts"
kind = "label"
icon = ""
path = ""

[[sidebar]]
name = "Tags"
icon = "tag-symbolic"
path = "tags://"

[[sidebar]]
name = "Search"
icon = "system-search-symbolic"
path = "search://"

[[sidebar]]
name = "Actions"
icon = "application-x-executable-symbolic"
path = "actions://"
"#,
        );

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

        let mut add_entry = |path: PathBuf, icon: &str, custom_name: Option<&str>| {
            if path.exists() || icon == "user-trash-symbolic" {
                let name = custom_name.map(|s| s.to_string()).unwrap_or_else(|| {
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Unknown".into())
                });

                let path_str = if path == home {
                    "~".to_string()
                } else if icon == "user-trash-symbolic" {
                    "trash:///".to_string()
                } else if let Ok(stripped) = path.strip_prefix(&home) {
                    format!("~/{}", stripped.to_string_lossy())
                } else {
                    path.to_string_lossy().into_owned()
                };

                use std::fmt::Write;
                let _ = write!(
                    default_toml,
                    "[[sidebar]]\nname = {:?}\nicon = {:?}\npath = {:?}\n\n",
                    name, icon, path_str
                );
            }
        };

        // 1. Home
        add_entry(home.clone(), "user-home-symbolic", Some("Home"));

        // 2. Localized XDG Folders with correct icons
        if let Some(p) = dirs::download_dir() {
            add_entry(p, "folder-download-symbolic", None);
        }
        if let Some(p) = dirs::document_dir() {
            add_entry(p, "folder-documents-symbolic", None);
        }
        if let Some(p) = dirs::picture_dir() {
            add_entry(p, "folder-pictures-symbolic", None);
        }
        if let Some(p) = dirs::video_dir() {
            add_entry(p, "folder-videos-symbolic", None);
        }
        if let Some(p) = dirs::audio_dir() {
            add_entry(p, "folder-music-symbolic", None);
        }

        // 3. Trash
        add_entry(
            PathBuf::from("trash:///"),
            "user-trash-symbolic",
            Some("Trash"),
        );

        let _ = fs::write(&config_path, default_toml);
    }

    let config_content = fs::read_to_string(&config_path).unwrap_or_default();

    let mut config: crate::model::Config = match toml::from_str(&config_content) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("[flux] CONFIG ERROR: Failed to parse config.toml: {}", e);
            crate::model::Config {
                ui: crate::model::UIConfig {
                    fish_theme: false,
                    auto_list_threshold: 300,
                    terminal: TerminalConfig::default(),
                    preview_syntax_highlighting: true,
                    wide_preview: false,
                    archive_thumbnail_cache: false,
                    folder_grid_thumbnails: true,
                    preview_panel_width: 400,
                    diff_editor: "nvim".to_string(),
                    ui_scale: 1.0,
                    bg_alpha_window: 0.65,
                    bg_alpha_sidebar_left: 0.65,
                    bg_alpha_sidebar_right: 0.65,
                    scale_font_with_icons: false,
                    hidden_extensions: Vec::new(),
                    tag_panel_width: 350,
                    actions_panel_width: 380,
                    search_panel_width: 350,
                    auto_show_diff: true,
                    diff_panel_width: 420,
                    show_symlink_emblem: true,
                    header_visible: true,
                    header_show_filter: true,
                    header_show_sort: true,
                    header_show_view: true,
                    header_show_preview: true,
                    sidebar_menu_requires_ctrl: false,
                    panel_menu_requires_ctrl: false,
                    auto_mime_body_color: "#e4e4e4".to_string(),
                    auto_mime_font_color: "#ffffff".to_string(),
                    auto_generate_mime_icons: true,
                    auto_mime_accent_color: "#1273b2".to_string(),
                    auto_mime_font_size: 9.0,
                    show_empty_dir_emblem: false,
                    default_icon_size: 128,
                    list_icon_size: 24,
                    startup_window_width: crate::ui::constants::DEFAULT_WIDTH,
                    startup_window_height: crate::ui::constants::DEFAULT_HEIGHT,
                    single_click: false,
                    show_csd: true,
                    sidebar_width: 240,
                    show_xdg_dirs: false,
                    current_folders_first: std::collections::HashMap::new(),
                    default_sort: crate::model::SortBy::Name,
                    folder_sort: std::collections::HashMap::new(),
                    folder_icon_size: std::collections::HashMap::new(),
                    show_hidden_by_default: false,
                    device_renames: std::collections::HashMap::new(),
                    folders_first: true,
                    theme: Some("default".to_string()),
                    start_maximized: true,
                    max_width_chars: 20,
                    grid_spacing: 10,
                    ascending: true,
                    expand_labels: false,
                    folder_icons: std::collections::HashMap::new(),
                    file_icons: std::collections::HashMap::new(),
                    sidebar_visible: true,
                    show_recents: true,
                    recents_row: 0,
                    show_thumbnails: true,
                    thumbnail_types: crate::model::ThumbnailTypes::default(),
                    thumbnail_size: 256,
                    max_content_search_results:
                        crate::services::constants::MAX_CONTENT_SEARCH_RESULTS,
                    lazy_thumbnails: false,
                    disable_drag_and_drop: false,
                    loader_batch_size: 50,
                    folder_cache_capacity: 3,
                    thumbnail_threads: 4,
                    max_search_results: 5000,
                    max_history: 100,
                    breadcrumb_count: 3,
                    breadcrumb_max_chars: 20,
                    ffmpeg_threads: 1,
                    ffmpeg_seek_seconds: 5.0,
                    ffmpeg_auto_rotate: false,
                    window_controls_left: false,
                    autoplay_video_previews: true,
                    enable_file_indexing: false,
                    content_search_max_file_mb: 128,
                },
                sidebar: vec![],
                shortcuts: crate::model::ShortcutsConfig::default(),
                network_bookmarks: vec![],
                default_list_mode: false,
            }
        }
    };

    // One-shot: migrate the legacy `ui.terminal_in_sidebar` bool into
    // `ui.terminal.position`. Runs before the sidebar names are translated, so
    // the rewritten file keeps the untranslated names.
    if migrate_legacy_terminal_position(&mut config, &config_content) {
        // Rewrites config.toml without the old key.
        save_config(&config);
        // `save_config` primed the cache with this pre-translation copy. Drop it so
        // `load_config` caches the final config below instead.
        invalidate_config_cache();
    }

    // Translate standard sidebar headers and items dynamically
    for item in &mut config.sidebar {
        item.name = match item.name.as_str() {
            "Default" => tr("Default"),
            "Shortcuts" => tr("Shortcuts"),
            "Tags" => tr("Tags"),
            "Search" => tr("Search"),
            "Actions" => tr("Actions"),
            other => other.to_string(),
        };
    }

    let mut changed = false;

    config.ui.folder_sort.retain(|path_str, _| {
        let path = if path_str.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path_str.trim_start_matches("~/")))
                .unwrap_or_else(|| PathBuf::from(path_str))
        } else {
            PathBuf::from(path_str)
        };

        let exists = path.exists() || path_str == "trash:///";
        if !exists {
            changed = true;
        }
        exists
    });

    config.ui.folder_icon_size.retain(|path_str, _| {
        let path = if path_str.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path_str.trim_start_matches("~/")))
                .unwrap_or_else(|| PathBuf::from(path_str))
        } else {
            PathBuf::from(path_str)
        };
        let exists = path.exists() || path_str == "trash:///";
        if !exists {
            changed = true;
        }
        exists
    });

    if changed {
        save_config(&config);
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Config;

    /// Parses `raw` like `load_config_from_disk`, migrates, and returns the config.
    fn load(raw: &str) -> (Config, bool) {
        let mut config: Config = toml::from_str(raw).unwrap();
        let rewrite = migrate_legacy_terminal_position(&mut config, raw);
        (config, rewrite)
    }

    #[test]
    fn legacy_terminal_in_sidebar_true_migrates_to_sidebar_and_drops_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[ui]\nterminal_in_sidebar = true\n").unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        let (config, rewrite) = load(&raw);
        assert!(rewrite);
        assert_eq!(config.ui.terminal.position, TerminalPosition::Sidebar);

        // `save_config` writes exactly `toml::to_string_pretty(&config)`.
        std::fs::write(&path, toml::to_string_pretty(&config).unwrap()).unwrap();
        let rewritten = std::fs::read_to_string(&path).unwrap();
        assert!(
            !rewritten.contains("terminal_in_sidebar"),
            "legacy key survived the rewrite:\n{rewritten}"
        );
        assert!(rewritten.contains("position = \"sidebar\""));

        // One-shot: the rewritten file has nothing left to migrate.
        let (reloaded, rewrite_again) = load(&rewritten);
        assert!(!rewrite_again);
        assert_eq!(reloaded.ui.terminal.position, TerminalPosition::Sidebar);
    }

    #[test]
    fn legacy_terminal_in_sidebar_false_stays_bottom_but_still_drops_the_key() {
        let (config, rewrite) = load("[ui]\nterminal_in_sidebar = false\n");
        assert!(rewrite);
        assert_eq!(config.ui.terminal.position, TerminalPosition::Bottom);
        assert!(!toml::to_string_pretty(&config)
            .unwrap()
            .contains("terminal_in_sidebar"));
    }

    #[test]
    fn missing_legacy_key_needs_no_rewrite() {
        let (config, rewrite) = load("[ui]\n");
        assert!(!rewrite);
        assert_eq!(config.ui.terminal.position, TerminalPosition::Bottom);
    }

    #[test]
    fn an_explicit_position_wins_over_the_legacy_key() {
        let (config, rewrite) =
            load("[ui]\nterminal_in_sidebar = true\n\n[ui.terminal]\nposition = \"fullscreen\"\n");
        assert!(rewrite);
        assert_eq!(config.ui.terminal.position, TerminalPosition::Fullscreen);
    }

    #[test]
    fn invalid_toml_is_not_migrated() {
        assert_eq!(
            legacy_terminal_in_sidebar("[ui\nterminal_in_sidebar = true"),
            None
        );
    }
}
