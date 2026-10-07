use crate::i18n::tr;
use crate::model::TerminalConfig;
use std::fs;
use std::path::PathBuf;

use super::cache::save_config;

pub(super) fn load_config_from_disk() -> crate::model::Config {
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
                    diff_editor: "nvim".to_string(),
                    ui_scale: 1.0,
                    bg_alpha_window: 0.65,
                    bg_alpha_sidebar_left: 0.65,
                    bg_alpha_sidebar_right: 0.65,
                    scale_font_with_icons: false,
                    hidden_extensions: Vec::new(),
                    tag_panel_width: 350,
                    search_panel_width: 350,
                    auto_show_diff: true,
                    diff_panel_width: 420,
                    show_symlink_emblem: true,
                    header_visible: true,
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
                    terminal: TerminalConfig::default(),
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

    // Translate standard sidebar headers and items dynamically
    for item in &mut config.sidebar {
        item.name = match item.name.as_str() {
            "Default" => tr("Default"),
            "Shortcuts" => tr("Shortcuts"),
            "Tags" => tr("Tags"),
            "Search" => tr("Search"),
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
