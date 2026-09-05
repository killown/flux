use crate::model::CustomAction;
use crate::model::MenuEntry;
use crate::ui::constants;
use crate::utils::PathExt;
use adw::gdk;
use adw::prelude::*;
use gtk::gdk_pixbuf;
use gtk::gio;
use gtk::glib;
use oxipng::{Options, StripChunks};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::TerminalConfig;

thread_local! {
    static THEMED_ICON_CACHE: RefCell<HashMap<String, adw::gio::Icon>> = RefCell::new(HashMap::new());
}

pub fn ensure_config_file() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    let config_path = config_dir.join("menu.rs");
    if !config_path.exists() {
        // Syntax: "Label" => "mime_types", "command", "Optional Toast Message"
        // - Use %p for file path, %d for directory, %f for filename.
        // - The third argument (Toast) is optional and shows a notification after execution.

        let default_config = r#"
# --- Core Operations ---
"󰋼      Add to Quick List" => "directory", "builtin::add_to_quick_list"
"󰆏      Copy" => "all", "builtin::copy", "Copied to clipboard"
"󰆐      Cut" => "all", "builtin::cut", "Cut to clipboard"
"󰏊      Paste" => "all", "builtin::paste", "Pasted items"
"󰑕      Rename" => "all", "builtin::rename"
"󰱝      Open With..." => "file", "builtin::open_with"
"🆃      Edit Tags" => "file", "builtin::tagfile", "Opening tag editor..."
"󰩹      Move to Trash" => "all", "gio trash %p", "Moved to trash"
"󰦬      Restore File" => "trash", "gio trash --restore %p", "File restored"
"󰆴      Shred File (Permanent)" => "all", "python $HOME/.local/share/flux/scripts/flux_shredder.py %p", "Shredder initialized"
"󰛖      Compress > To ZIP" => "all", "/usr/bin/python $HOME/.local/share/flux/scripts/flux_simple_compressor.py %p", "Compressing to ZIP..."

# --- Navigation & System ---
"      Open Terminal" => "directory", "alacritty --working-directory=%p"
"󰋜      Toggle Pin" => "directory", "builtin::toggle_pin", "Sidebar updated"
"󰨞      Open in VSCode" => "text/all, application/all", "code %p"
"󰋽      File Properties" => "file", "flux-fm --file-properties %p"
"󰋊      Folder Info" => "directory", "baobab %p"
"󰉋      New Folder" => "directory", "builtin::new_folder", "Folder created"
"󰈔      New File" => "directory", "builtin::new_file", "File created"
"󰸉      Set Custom Icon" => "all", "builtin::set_custom_icon", "Custom icon set"
"󰸉      Reset Custom Icon" => "all", "builtin::reset_custom_icon", "Custom icon Reseted"

# --- Media Edit ---
"󰽰      Media Edit > Join Videos" => "video/all", "python3 $HOME/.local/share/flux/scripts/join_videos.py %p", "Joining videos..."
"󰽰      Media Edit > Cut Video" => "video/all", "python $HOME/.local/share/flux/scripts/video_cutter.py %p", "Opening Video Cutter..."
"󰽰      Media Edit > Mix Audio" => "audio/", "python3 $HOME/.local/share/flux/scripts/mix_audio.py %p", "Mixing audio..."
"󰽰      Media Edit > Merge Video + Audio" => "video/all+audio/all", "python3 $HOME/.local/share/flux/scripts/join_mp4_mp3.py %p", "Merging media..."

# --- Media Convert ---
"󰽰      Media Convert > To MP4" => "video/all", "ffmpeg -i %p -codec copy %p.mp4", "Converting to MP4..."
"󰽰      Media Convert > To MKV" => "video/all", "ffmpeg -i %p -codec copy %p.mkv", "Converting to MKV..."
"󰽰      Media Convert > To WebM" => "video/all", "ffmpeg -i %p -codec copy %p.webm", "Converting to WebM..."
"󰽰      Media Convert > To MOV" => "video/all", "ffmpeg -i %p -codec copy %p.mov", "Converting to MOV..."
"󰝚      Media Convert > Audio to MP3" => "audio/x-opus+ogg, audio/vnd.wave, audio/ogg", "ffmpeg -i %p -vn -ab 192k -ar 44100 -y %p.mp3", "Converting to MP3..."

# --- Media Optimization & Extraction ---
"󰠝      Media Extract > MP3 from Video" => "video/all", "ffmpeg -i %p -vn -acodec libmp3lame -q:a 2 %p.mp3", "Extracting MP3..."
"󰕧      Media Optimize > Reduce Video Size" => "video/all", "ffmpeg -i %p -vcodec libx265 -crf 28 -tag:v hvc1 -preset faster %p_reduced.mp4", "Reducing video size..."
"󰛖      Extract Here!" => "application/zip, application/x-7z-compressed, application/x-rar, application/x-tar", "7z x %p -o%p_extracted", "Extracting archive..."

# --- Images & Wallpaper ---
"󰸉      Image Wallpaper > Set (swww)" => "image/all", "swww img %p", "Wallpaper set (swww)"
"󰸉      Image Wallpaper > Set (wbg)" => "image/all", "cp %p ~/Images/fav.jpg && wbg -s ~/Images/fav.jpg", "Wallpaper set (wbg)"
"󰸉      Image Convert > To AVIF" => "image/all", "avifenc --jobs all -q 65 %p %p.avif", "Image converted to AVIF"
"󰸉      Image Convert > To JPG" => "image/all", "magick %p -quality 75 -strip %p-output.jpg", "Image converted to JPG"
"󰏦      Convert to PDF" => "image/all, application/pdf, application/msword, application/vnd.openxmlformats-officedocument.wordprocessingml.document", "python3 $HOME/.local/share/flux/scripts/pdf_converter.py %p", "Converting to PDF..."

# --- Tools ---
"󰯦      Tools > Git Gui" => "directory", "git gui"
"󰯦      Tools > Download Video (1080p)" => "directory", "cd %p && yt-dlp -f 'bv[height<=1080]+ba/b[height<=1080]' $(wl-paste)", "Video download started"
"󰯦      Tools > Copy Path" => "all", "echo -n %p | wl-copy", "Path copied to clipboard"
"󰯦      Tools > Copy Name" => "all", "basename %p | tr -d '\n' | wl-copy", "Name copied to clipboard"
"󰯦      Tools > Advanced Archive Manager" => "all", "/usr/bin/python $HOME/.local/share/flux/scripts/flux_compressor.py %p"
"#;

        if let Ok(mut file) = fs::File::create(&config_path) {
            let _ = file.write_all(default_config.as_bytes());
        }
    }
    config_path
}

pub fn save_config(config: &crate::model::Config) {
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

pub fn rename_path(old_path: &Path, new_name: &str) -> std::io::Result<PathBuf> {
    // Reject any name containing a path separator to prevent directory traversal
    if new_name.contains(std::path::MAIN_SEPARATOR) || new_name.contains('/') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "new name must be a plain filename, not a path",
        ));
    }

    let mut new_path = old_path.to_path_buf();
    new_path.set_file_name(new_name);

    if new_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "A file with this name already exists",
        ));
    }

    fs::rename(old_path, &new_path)?;
    Ok(new_path)
}

pub fn load_config() -> crate::model::Config {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        let _ = fs::create_dir_all(&config_dir);

        let mut default_toml = String::from(
            r#"[ui]
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
show_csd = false
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
name = "Tags"
icon = "tag-symbolic"
path = "tags://"

[[sidebar]]
name = "Search"
icon = "system-search-symbolic"
path = "search://"

[[sidebar]]
name = "Home"
icon = "user-home-symbolic"
path = "~"
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
                    max_content_search_results:
                        crate::services::constants::MAX_CONTENT_SEARCH_RESULTS,
                    lazy_thumbnails: false,
                    disable_drag_and_drop: false,
                    loader_batch_size: 50,
                    folder_cache_capacity: 3,
                    thumbnail_threads: 4,
                    max_search_results: 5000,
                    max_history: 100,
                },
                sidebar: vec![],
                shortcuts: crate::model::ShortcutsConfig::default(),
                network_bookmarks: vec![],
                default_list_mode: false,
            }
        }
    };
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

/// Parses the right-hand side of a menu config line into (mime, command, optional_toast).
pub fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>, bool)> {
    let input = input.trim();

    let remainder = input.strip_prefix('"')?;
    let (mime, rest) = remainder.split_once('"')?;

    let second_part = rest.trim().strip_prefix(',')?.trim();

    let cmd_inner = second_part.strip_prefix('"')?;
    let (cmd, after_cmd) = cmd_inner.split_once('"')?;

    // Parse remaining optional tokens: toast and/or "no_command_dialog" (in any order)
    let mut toast: Option<String> = None;
    let mut no_command_dialog = false;

    let mut remainder = after_cmd.trim();
    while let Some(stripped) = remainder.strip_prefix(',') {
        let stripped = stripped.trim();
        if let Some(inner) = stripped.strip_prefix('"') {
            if let Some((token, rest)) = inner.split_once('"') {
                if token == "no_command_dialog" {
                    no_command_dialog = true;
                } else {
                    toast = Some(token.to_string());
                }
                remainder = rest.trim();
                continue;
            }
        }
        break;
    }

    Some((mime.to_string(), cmd.to_string(), toast, no_command_dialog))
}

pub fn load_menu_config() -> Vec<CustomAction> {
    let config_path = ensure_config_file();
    let content = std::fs::read_to_string(config_path).unwrap_or_default();

    let mut actions = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        if let Some((left, right)) = line.split_once("=>") {
            let full_label = left.trim().trim_matches('"');

            let (submenu, label) = if full_label.contains(" > ") {
                let parts: Vec<&str> = full_label.splitn(2, " > ").collect();
                (Some(parts[0].to_string()), parts[1].to_string())
            } else {
                (None, full_label.to_string())
            };

            if let Some((mimes_part, cmd_part, toast, no_command_dialog)) = split_mime_cmd(right) {
                let mime_types: Vec<String> = mimes_part
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();

                actions.push(CustomAction {
                    label,
                    submenu,
                    action_name: format!("custom_{}", i),
                    command: cmd_part,
                    mime_types,
                    toast,
                    no_command_dialog,
                });
            }
        }
    }
    actions
}

/// Writes the current menu actions to `~/.config/flux/menu.rs`
/// in the same DSL format expected by `load_menu_config`.
pub fn save_menu_config(actions: &[CustomAction]) -> std::io::Result<()> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");
    std::fs::create_dir_all(&config_dir)?;
    let path = config_dir.join("menu.rs");

    let mut content = String::new();
    for action in actions {
        // Convert CustomAction to MenuEntry for consistent serialization
        let entry = MenuEntry {
            label: action.label.clone(),
            submenu: action.submenu.clone(),
            mime_types: action.mime_types.join(", "),
            command: action.command.clone(),
            toast: action.toast.clone(),
            no_command_dialog: action.no_command_dialog,
        };
        content.push_str(&entry.to_config_line());
        content.push('\n');
    }
    std::fs::write(path, content)?;
    Ok(())
}

pub fn get_icon_for_path(path: &Path, is_dir: bool) -> adw::gio::Icon {
    get_icon_for_path_with_override(path, is_dir, None)
}

/// Returns a GIO icon for the given path, applying a custom icon name override when provided.
///
/// # Arguments
///
/// * `path`          - Absolute path to the file or directory.
/// * `is_dir`        - Whether the entry is a directory.
/// * `custom_icon`   - Optional GTK icon name to use instead of the derived default.
pub fn get_icon_for_path_with_override(
    path: &Path,
    is_dir: bool,
    custom_icon: Option<&str>,
) -> adw::gio::Icon {
    if let Some(icon_name) = custom_icon {
        if let Ok(icon) = gio::Icon::for_string(icon_name) {
            return icon;
        }
    }
    if is_dir {
        return gio::Icon::for_string("folder").unwrap();
    }
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    // Fast-path: check static extension mapping before calling GIO content_type_guess.
    let content_type = if let Some(mime) = guess_mime_from_extension(&filename) {
        mime
    } else if crate::services::network::is_network_uri(path) {
        "application/octet-stream".to_string()
    } else {
        let (ct, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
        ct.to_string()
    };

    THEMED_ICON_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if let Some(icon) = map.get(&content_type) {
            return icon.clone();
        }
        let icon = adw::gio::content_type_get_icon(&content_type);
        map.insert(content_type, icon.clone());
        icon
    })
}

pub fn get_mime_type(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    if crate::services::network::is_network_uri(path) {
        if path_str.ends_with('/') {
            return "inode/directory".to_string();
        }

        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if filename.is_empty() {
            return "inode/directory".to_string();
        }

        let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), None);
        let ct = content_type.to_string();

        if ct == "inode/directory" {
            return guess_mime_from_extension(&filename)
                .unwrap_or_else(|| "application/octet-stream".to_string());
        }
        return ct;
    }

    if path.is_dir() {
        return "inode/directory".to_string();
    }

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let mut sniff_buffer = [0u8; 4096];
    let data_slice = if let Ok(mut file) = fs::File::open(path) {
        if let Ok(count) = file.read(&mut sniff_buffer) {
            Some(&sniff_buffer[..count])
        } else {
            None
        }
    } else {
        None
    };

    let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), data_slice);
    let ct = content_type.to_string();

    if ct == "inode/directory" && path.is_file() {
        return guess_mime_from_extension(&filename)
            .unwrap_or_else(|| "application/octet-stream".to_string());
    }

    ct
}

fn guess_mime_from_extension(filename: &str) -> Option<String> {
    let ext = std::path::Path::new(filename).extension()?.to_str()?;
    let mime = match ext.to_lowercase().as_str() {
        // Text & Data
        "txt" | "log" | "text" => "text/plain",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "json" => "application/json",
        "jsonld" => "application/ld+json",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "ini" | "conf" | "cfg" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "rtf" => "application/rtf",

        // Web & Scripts
        "html" | "htm" | "xhtml" => "text/html",
        "css" => "text/css",
        "js" | "mjs" | "cjs" => "text/javascript",
        "ts" | "mts" | "cts" => "text/typescript",
        "jsx" => "text/jsx",
        "tsx" => "text/tsx",
        "wasm" => "application/wasm",
        "php" => "application/x-httpd-php",

        // Source Code & Shell
        "py" | "pyw" => "text/x-python",
        "rs" => "text/x-rust",
        "c" | "h" => "text/x-c",
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => "text/x-c++",
        "go" => "text/x-go",
        "java" => "text/x-java-source",
        "sh" | "bash" | "zsh" => "application/x-sh",
        "sql" => "application/sql",
        "lua" => "text/x-lua",
        "diff" | "patch" => "text/x-diff",

        // Images
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "heic" | "heif" => "image/heic",
        "jxl" => "image/jxl",
        "svg" | "svgz" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "ico" => "image/vnd.microsoft.icon",

        // Audio
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "mid" | "midi" => "audio/midi",

        // Video
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "m2ts" => "video/mp2t",

        // Documents & Office (MS Office & OpenDocument)
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "epub" => "application/epub+zip",

        // Archives & Compression
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "tgz" => "application/gzip",
        "bz2" => "application/x-bzip2",
        "xz" => "application/x-xz",
        "zst" => "application/zstd",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/x-rar",
        "iso" | "img" => "application/x-cd-image",
        "deb" => "application/vnd.debian.binary-package",
        "rpm" => "application/x-rpm",

        // Fonts
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",

        _ => "application/octet-stream",
    };
    Some(mime.to_string())
}

pub fn is_visual_media(path: &Path) -> (bool, bool) {
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
    (
        content_type.starts_with("image/"),
        content_type.starts_with("video/"),
    )
}

pub fn expand_path(path: &str) -> PathBuf {
    // We delegate the logic to our PathExt trait which handles
    // component stripping and home directory joining safely.

    PathBuf::from(path).expand_tilde()
}

pub fn open_file(path: PathBuf) {
    let path_str = path.to_string_lossy();

    // Determine if this is a network URI
    let file = if crate::services::network::is_network_uri(&path) {
        gio::File::for_uri(&path_str)
    } else {
        gio::File::for_path(&path)
    };

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), None);
    if let Some(app_info) = gio::AppInfo::default_for_type(&content_type, false) {
        let _ = app_info.launch(&[file], None::<&gio::AppLaunchContext>);
    } else {
        // Fallback: if we have a URI, use xdg-open with the URI, else the path
        if crate::services::network::is_network_uri(&path) {
            let _ = Command::new("xdg-open").arg(&*path_str).spawn();
        } else {
            let _ = Command::new("xdg-open").arg(path).spawn();
        }
    }
}

/// Optimizes raw PNG bytes in-place using oxipng.
fn optimize_png_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut opts = Options::from_preset(2);
    opts.strip = StripChunks::All;

    oxipng::optimize_from_memory(bytes, &opts).unwrap_or_else(|_| bytes.to_vec())
}

/// Resolves the FreeDesktop-compliant thumbnail cache path for a given source file.
///
/// Implements §2 of the [Thumbnail Managing Standard] by computing an MD5 digest
/// of the canonical `file://` URI and mapping it into the shared XDG thumbnail
/// store at `$XDG_CACHE_HOME/thumbnails/<size-tier>/<hash>.png`. Cache entries
/// produced here are session-persistent and visible to other XDG-compliant
/// applications (e.g. Nautilus, Thunar) in the same store.
///
/// [Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to the source media file.
///
/// # Returns
///
/// `(cache_dir, cache_path)` where `cache_dir` is the resolved size-tier directory
/// and `cache_path` is the full `.png` destination. Returns `None` if
/// `dirs::cache_dir()` is unavailable or `path` contains non-UTF-8 bytes.
fn thumbnail_cache_path(path: &Path) -> Option<(PathBuf, PathBuf)> {
    let thumb_folder = match constants::CACHED_THUMBNAIL_SIZE {
        512 => "xx-large",
        256 => "x-large",
        128 => "large",
        _ => "normal",
    };

    let cache_dir = dirs::cache_dir()?.join("thumbnails").join(thumb_folder);

    // gio::File::uri() produces a correctly percent-encoded RFC 2396 URI,
    // which is what the FreeDesktop thumbnail spec mandates for the MD5 input.
    let uri = gio::File::for_path(path).uri();
    let hash = format!("{:x}", md5::compute(uri.as_bytes()));
    let cache_path = cache_dir.join(format!("{}.png", hash));

    Some((cache_dir, cache_path))
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Renders the first page of a PDF to a PNG thumbnail and writes it to `cache_path`.
///
/// Scales the page so its longest axis fits within [`constants::CACHED_THUMBNAIL_SIZE`],
/// paints a white background (PDF pages are transparent by default), then serialises
/// the result via `gdk_pixbuf` into the shared XDG thumbnail store so it is
/// session-persistent and reused on subsequent directory loads.
///
/// # Arguments
///
/// * `path`       - Absolute path to the source PDF file.
/// * `cache_path` - Destination `.png` path inside the XDG thumbnail store.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the document cannot be opened, contains
/// no pages, or the Cairo surface cannot be serialised to PNG.
fn pdf_thumbnail(path: &Path, cache_path: &Path) -> Option<gdk::Texture> {
    let doc = poppler::PopplerDocument::new_from_file(path, None).ok()?;
    let page = doc.get_page(0)?;
    let (page_w, page_h) = page.get_size();

    let size = constants::CACHED_THUMBNAIL_SIZE as f64;
    let scale = size / page_w.max(page_h);
    let render_w = (page_w * scale).round() as i32;
    let render_h = (page_h * scale).round() as i32;

    // ARgb32 gives us 4 bytes/pixel (BGRA native order) which matches what
    // `surface.data()` returns and what `Pixbuf::from_bytes` with has_alpha=true expects.
    let mut surface =
        poppler::cairo::ImageSurface::create(poppler::cairo::Format::ARgb32, render_w, render_h)
            .ok()?;
    let cx = poppler::cairo::Context::new(&surface).ok()?;

    // PDF pages are transparent by default, fill white so the thumbnail
    // looks correct on both light and dark file-manager backgrounds.
    cx.set_source_rgb(1.0, 1.0, 1.0);
    cx.paint().ok()?;
    cx.scale(scale, scale);
    page.render(&cx);

    // Drop the context before calling surface.data() - both borrow the surface
    // and Rust enforces that only one mutable borrow exists at a time.
    // Avoids a PNG encode/decode round-trip and sidesteps the `'static` bound
    // on `Pixbuf::from_read`. `ImageSurface::data()` exposes BGRA (cairo native),
    // so has_alpha=true lets gdk_pixbuf handle the channel layout via the stride.
    drop(cx);
    surface.flush();
    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride();
    let data = surface.data().ok()?;

    let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from(&*data),
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        stride,
    );

    if let Ok(buffer) = pixbuf.save_to_bufferv("png", &[("compression", "9")]) {
        let optimized = optimize_png_bytes(&buffer);
        let _ = std::fs::write(cache_path, optimized);
    }

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// Returns `true` if `path` is a font file by extension (case-insensitive).
fn is_font(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "ttf" | "otf" | "woff" | "woff2" | "ttc"
        )
    })
}

/// Renders a font preview thumbnail using PangoCairo and writes it to `cache_path`.
///
/// Loads the font file directly into a temporary [`pango::FontMap`] override via
/// `fc-cache`-free [`fontconfig`] font loading, then lays out a two-line sample:
/// the font family name on top and the pangram `"AaBbCc 123"` below in the target
/// font at a size scaled to fill [`constants::CACHED_THUMBNAIL_SIZE`].
///
/// The background is white with dark text so thumbnails are legible on both light
/// and dark file-manager themes.
///
/// # Arguments
///
/// * `path`       - Absolute path to the source font file.
/// * `cache_path` - Destination `.png` path inside the XDG thumbnail store.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the font cannot be loaded or the Cairo
/// surface cannot be read back.
/// Reads the font family name from a TrueType/OpenType `name` table (nameID 1).
///
/// Parses just enough of the binary `name` table to extract the English family
/// name without pulling in a full font parsing library.  Returns `None` if the
/// file cannot be read or the table is malformed, the caller falls back to the
/// filename stem in that case.
fn read_font_family(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;

    // Offset table: 12 bytes header + 16 bytes per table record.
    // We scan the table directory for the 'name' tag (0x6E616D65).
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([*data.get(4)?, *data.get(5)?]) as usize;
    let dir_start = 12usize;

    let mut name_offset = None;
    for i in 0..num_tables {
        let base = dir_start.checked_add(i.checked_mul(16)?)?;
        if base.checked_add(16)? > data.len() {
            break;
        }
        let tag = data.get(base..base + 4)?;
        if tag == b"name" {
            let offset = u32::from_be_bytes([
                *data.get(base + 8)?,
                *data.get(base + 9)?,
                *data.get(base + 10)?,
                *data.get(base + 11)?,
            ]) as usize;
            name_offset = Some(offset);
            break;
        }
    }

    let name_base = name_offset?;
    if name_base.checked_add(6)? > data.len() {
        return None;
    }

    let count = u16::from_be_bytes([*data.get(name_base + 2)?, *data.get(name_base + 3)?]) as usize;
    let string_offset =
        u16::from_be_bytes([*data.get(name_base + 4)?, *data.get(name_base + 5)?]) as usize;
    let storage = name_base.checked_add(string_offset)?;

    // Scan name records (12 bytes each) for nameID=1 (Family), platformID=3 (Windows), encodingID=1 (Unicode BMP).
    // Fall back to platformID=1 (Mac) if no Windows record found.
    let mut family_win: Option<String> = None;
    let mut family_mac: Option<String> = None;

    for i in 0..count {
        let rec = name_base.checked_add(6)?.checked_add(i.checked_mul(12)?)?;
        if rec.checked_add(12)? > data.len() {
            break;
        }
        let platform_id = u16::from_be_bytes([*data.get(rec)?, *data.get(rec + 1)?]);
        let encoding_id = u16::from_be_bytes([*data.get(rec + 2)?, *data.get(rec + 3)?]);
        let name_id = u16::from_be_bytes([*data.get(rec + 6)?, *data.get(rec + 7)?]);
        let length = u16::from_be_bytes([*data.get(rec + 8)?, *data.get(rec + 9)?]) as usize;
        let offset = u16::from_be_bytes([*data.get(rec + 10)?, *data.get(rec + 11)?]) as usize;

        if name_id != 1 {
            continue;
        }

        let start = storage.checked_add(offset)?;
        let end = start.checked_add(length)?;
        if end > data.len() {
            continue;
        }
        let raw = data.get(start..end)?;

        if platform_id == 3 && encoding_id == 1 && family_win.is_none() {
            // UTF-16 BE
            #[allow(clippy::chunks_exact_to_as_chunks)]
            let chars: Vec<u16> = raw
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&chars) {
                family_win = Some(s);
            }
        } else if platform_id == 1 && family_mac.is_none() {
            // Mac Roman - ASCII-compatible for Latin family names
            family_mac = Some(String::from_utf8_lossy(raw).into_owned());
        }
    }

    family_win.or(family_mac)
}

/// Registers a font file with the process-local fontconfig instance so Pango
/// can resolve it without system installation.
///
/// Calls `FcConfigAppFontAddFile` from libfontconfig (a transitive system
/// dependency of GTK/Pango - always present on the target platform).  The
/// registration is process-local and cleaned up on exit, no global state is
/// modified.
fn register_font_with_fontconfig(path: &Path) {
    // SAFETY: libfontconfig is guaranteed present (GTK transitive dep).
    // FcConfigAppFontAddFile(NULL, path) adds `path` to the default config's
    // application font list.  NULL config pointer → current default config.
    // The path string is valid for the duration of the call.
    #[link(name = "fontconfig")]
    extern "C" {
        fn FcConfigAppFontAddFile(
            config: *mut std::ffi::c_void,
            file: *const std::os::raw::c_char,
        ) -> i32;
    }

    if let Ok(cpath) = std::ffi::CString::new(path.to_string_lossy().as_bytes()) {
        unsafe {
            FcConfigAppFontAddFile(std::ptr::null_mut(), cpath.as_ptr());
        }
    }
}

fn font_thumbnail(path: &Path, cache_path: &Path) -> Option<gdk::Texture> {
    // Register the font file with fontconfig so Pango can load it by family
    // name without requiring system installation.
    register_font_with_fontconfig(path);

    // Prefer the actual internal family name from the binary name table,
    // fall back to the filename stem if parsing fails.
    let family = read_font_family(path)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Sans".to_string());

    let size = constants::CACHED_THUMBNAIL_SIZE;
    let size_f = size as f64;

    let mut surface =
        pangocairo::cairo::ImageSurface::create(pangocairo::cairo::Format::ARgb32, size, size)
            .ok()?;
    let cx = pangocairo::cairo::Context::new(&surface).ok()?;

    cx.set_source_rgb(1.0, 1.0, 1.0);
    cx.paint().ok()?;

    let font_map = pangocairo::FontMap::default();
    let pango_cx = font_map.create_context();
    pangocairo::functions::update_context(&cx, &pango_cx);

    // Sample lines: family name hint at top, pangram below.
    let sample_body = "AaBbCc 123\nThe quick fox";
    let body_pt = (size_f * 0.22) as i32;
    let mut body_desc = gtk::pango::FontDescription::new();
    body_desc.set_family(&family);
    body_desc.set_size(body_pt * gtk::pango::SCALE);

    // Small label at the top: family name in a neutral sans so it's always legible.
    let label_pt = (size_f * 0.09) as i32;
    let mut label_desc = gtk::pango::FontDescription::new();
    label_desc.set_family("Sans");
    label_desc.set_size(label_pt * gtk::pango::SCALE);

    let margin = size_f * 0.06;

    // Draw the family name label.
    cx.set_source_rgb(0.4, 0.4, 0.4);
    cx.move_to(margin, margin);
    let label_layout = gtk::pango::Layout::new(&pango_cx);
    label_layout.set_font_description(Some(&label_desc));
    label_layout.set_text(&family);
    label_layout.set_width((size - (margin * 2.0) as i32) * gtk::pango::SCALE);
    label_layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &label_layout);

    // Draw the pangram sample in the target font.
    cx.set_source_rgb(0.05, 0.05, 0.05);
    let (_, label_h) = label_layout.pixel_size();
    cx.move_to(margin, margin + label_h as f64 + margin * 0.25);
    let body_layout = gtk::pango::Layout::new(&pango_cx);
    body_layout.set_font_description(Some(&body_desc));
    body_layout.set_text(sample_body);
    body_layout.set_width((size - (margin * 2.0) as i32) * gtk::pango::SCALE);
    body_layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &body_layout);

    drop(cx);
    surface.flush();

    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride();
    let data = surface.data().ok()?;

    let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from(&*data),
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        stride,
    );

    if let Ok(buffer) = pixbuf.save_to_bufferv("png", &[("compression", "9")]) {
        let optimized = optimize_png_bytes(&buffer);
        let _ = std::fs::write(cache_path, optimized);
    }

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// Returns whether a cached thumbnail PNG is still valid for the given source file.
///
/// Compares the source's `mtime` against the thumbnail's own `mtime`. A thumbnail
/// is considered stale when the source was modified after the thumbnail was written,
/// or when the on-disk entry is zero bytes (partial write / crash residue).
///
/// # Arguments
///
/// * `cache_path`   - Path to the candidate `.png` thumbnail.
/// * `source_meta`  - [`fs::Metadata`] of the original source file.
fn thumbnail_is_valid(cache_path: &Path, source_meta: &fs::Metadata) -> bool {
    let cache_meta = match fs::metadata(cache_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let cache_mtime = cache_meta.modified().ok();
    let source_mtime = source_meta.modified().ok();

    // Source newer than cache → stale.
    if let (Some(ct), Some(st)) = (cache_mtime, source_mtime) {
        if st > ct {
            return false;
        }
    }

    // Guard against zero-byte truncation or partial writes.
    cache_meta.len() > 0
}

/// Removes recent entries from the `~/.local/share/recently-used.xbel` file.
///
/// If `paths` is `None`, all bookmarks are removed.
/// If `Some(paths)`, only bookmarks whose `href` matches one of the given paths are removed.
///
/// # Errors
/// Returns an I/O error if the XBEL file cannot be read or written.
pub fn remove_recents(paths: Option<&[PathBuf]>) -> std::io::Result<()> {
    let xbel_path = dirs::data_local_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No data local dir"))?
        .join("recently-used.xbel");

    if !xbel_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&xbel_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut keep = Vec::new();

    if let Some(paths) = paths {
        // Build a set of canonical file:// URIs to match against XBEL's href attributes.
        let uris_to_remove: HashSet<String> = paths
            .iter()
            .map(|p| gio::File::for_path(p).uri().into())
            .collect();

        for line in lines {
            if line.trim_start().starts_with("<bookmark") {
                let should_remove = uris_to_remove.iter().any(|uri| line.contains(uri.as_str()));
                if !should_remove {
                    keep.push(line);
                }
            } else {
                keep.push(line);
            }
        }
    } else {
        // Remove all bookmarks: keep everything except `<bookmark …>` lines.
        for line in lines {
            if !line.trim_start().starts_with("<bookmark") {
                keep.push(line);
            }
        }
    }

    std::fs::write(&xbel_path, keep.join("\n"))?;
    Ok(())
}

/// Retrieves a thumbnail [`gdk::Texture`] for a visual media file, generating
/// and caching it on first access.
///
/// Cache entries follow the [FreeDesktop Thumbnail Managing Standard], stored under
/// `$XDG_CACHE_HOME/thumbnails/<size-tier>/` as `MD5("file://<path>").hex + ".png"`.
/// This makes cache hits session-persistent: a thumbnail written in a previous
/// session is reused without regeneration as long as the source file's `mtime` and
/// size have not changed.
///
/// Stale entries (source modified after thumbnail was written, or zero-byte files)
/// are evicted and regenerated atomically.
///
/// [FreeDesktop Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to an image or video file.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the file is not visual media, if any
/// required external tool (`ffmpeg`) is unavailable, or if I/O fails.
pub async fn get_or_create_thumbnail(path: &Path) -> Option<gdk::Texture> {
    let path_str = path.to_string_lossy();

    if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
        if let Some((archive_path, inner)) = crate::services::archive::parse_archive_uri(&path_str)
        {
            let tmp_file = tokio::task::spawn_blocking(move || {
                crate::services::archive::extract_entry_to_tempfile(&archive_path, &inner, None)
            })
            .await
            .ok()?
            .ok()?;

            let texture = Box::pin(get_or_create_thumbnail(tmp_file.path())).await;
            return texture;
        }
        return None;
    }

    let config = load_config();
    if !config.ui.show_thumbnails {
        return None;
    }

    let is_pdf_file = is_pdf(path);
    let is_font_file = !is_pdf_file && is_font(path);
    let (is_img, is_vid) = if !is_pdf_file && !is_font_file {
        is_visual_media(path)
    } else {
        (false, false)
    };

    let is_supported = (is_pdf_file && config.ui.thumbnail_types.pdfs)
        || (is_font_file && config.ui.thumbnail_types.fonts)
        || (is_img && config.ui.thumbnail_types.images)
        || (is_vid && config.ui.thumbnail_types.videos);

    if !is_supported {
        return None;
    }

    let (cache_dir, cache_path) = thumbnail_cache_path(path)?;

    // ── 1. Cache Check with Auto-Eviction of Corrupted Files ─────────────────
    let source_meta = tokio::fs::metadata(path).await.ok();
    if cache_path.exists() {
        if let Ok(meta) = tokio::fs::metadata(&cache_path).await {
            if meta.len() > 1024 * 1024 {
                let _ = tokio::fs::remove_file(&cache_path).await;
            }
        }

        let is_valid = source_meta
            .as_ref()
            .map(|m| thumbnail_is_valid(&cache_path, m))
            .unwrap_or(true);

        if is_valid && cache_path.exists() {
            if let Ok(bytes) = tokio::fs::read(&cache_path).await {
                let glib_bytes = glib::Bytes::from(&bytes);
                if let Ok(texture) = gdk::Texture::from_bytes(&glib_bytes) {
                    return Some(texture);
                }
            }
        }
        let _ = tokio::fs::remove_file(&cache_path).await;
    }

    tokio::fs::create_dir_all(&cache_dir).await.ok()?;

    // ── 2. Generation via Atomic Temp File ────────────────────────────────────
    if is_pdf_file {
        let cache_p = cache_path.clone();
        let path_p = path.to_path_buf();
        return tokio::task::spawn_blocking(move || pdf_thumbnail(&path_p, &cache_p))
            .await
            .ok()?;
    }

    if is_font_file {
        let cache_p = cache_path.clone();
        let path_p = path.to_path_buf();
        return tokio::task::spawn_blocking(move || font_thumbnail(&path_p, &cache_p))
            .await
            .ok()?;
    }

    if is_img {
        let path_buf = path.to_path_buf();
        let cache_p = cache_path.clone();
        let cache_d = cache_dir.clone();

        return tokio::task::spawn_blocking(move || {
            let max_dim = constants::CACHED_THUMBNAIL_SIZE;
            let pixbuf =
                gdk_pixbuf::Pixbuf::from_file_at_scale(&path_buf, max_dim, max_dim, true).ok()?;

            let width = pixbuf.width();
            let height = pixbuf.height();
            let pixbuf = if width > max_dim || height > max_dim {
                pixbuf.scale_simple(max_dim, max_dim, gdk_pixbuf::InterpType::Bilinear)?
            } else {
                pixbuf
            };

            let pid = std::process::id();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let tmp_path = cache_d.join(format!(".tmp.{pid}.{nanos}.png"));

            let written = if let Ok(buffer) = pixbuf.save_to_bufferv("png", &[("compression", "9")])
            {
                let optimized = optimize_png_bytes(&buffer);
                if std::fs::write(&tmp_path, optimized).is_ok() {
                    std::fs::rename(&tmp_path, &cache_p).is_ok()
                } else {
                    let _ = std::fs::remove_file(&tmp_path);
                    false
                }
            } else {
                false
            };

            drop(pixbuf);

            if written {
                if let Ok(bytes) = std::fs::read(&cache_p) {
                    let glib_bytes = glib::Bytes::from(&bytes);
                    return gdk::Texture::from_bytes(&glib_bytes).ok();
                }
            }

            None
        })
        .await
        .ok()?;
    }

    if is_vid {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = cache_dir.join(format!(".tmp.{pid}.{nanos}.png"));

        async fn try_ffmpeg(path: &Path, out_path: &Path, seek: &str) -> bool {
            tokio::process::Command::new("ffmpeg")
                .arg("-y")
                .arg("-loglevel")
                .arg("panic")
                .arg("-noautorotate")
                .arg("-ss")
                .arg(seek)
                .arg("-i")
                .arg(path)
                .arg("-an")
                .arg("-threads")
                .arg("1")
                .arg("-vframes")
                .arg("1")
                .arg("-vf")
                .arg(format!(
                    "scale={}:-1:force_original_aspect_ratio=decrease",
                    constants::CACHED_THUMBNAIL_SIZE
                ))
                .arg(out_path)
                .status()
                .await
                .map(|s| s.success())
                .unwrap_or(false)
        }

        let mut success = try_ffmpeg(path, &tmp_path, "5.000").await;
        if !success || !tmp_path.exists() {
            success = try_ffmpeg(path, &tmp_path, "0.000").await;
        }

        if success && tmp_path.exists() {
            if let Ok(raw_png) = tokio::fs::read(&tmp_path).await {
                let optimized = optimize_png_bytes(&raw_png);
                let _ = tokio::fs::write(&tmp_path, optimized).await;
            }

            let _ = tokio::fs::rename(&tmp_path, &cache_path).await;
            if let Ok(bytes) = tokio::fs::read(&cache_path).await {
                let glib_bytes = glib::Bytes::from(&bytes);
                return gdk::Texture::from_bytes(&glib_bytes).ok();
            }
        } else {
            let _ = tokio::fs::remove_file(&tmp_path).await;
        }
    }

    None
}

/// Helper: Resolves the original image filename if `dev_node` is a LUKS mapper backed by a loop device.
fn resolve_luks_loop_name(_dev_node: &str) -> Option<String> {
    // Check if sysfs knows which loop device backs this mapper/dm node
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("loop") {
                continue;
            }

            // Read the backing file for the loop device
            let backing_path = format!("/sys/block/{name_str}/loop/backing_file");
            if let Ok(backing) = fs::read_to_string(backing_path) {
                let backing_trimmed = backing.trim();
                let backing_path_buf = PathBuf::from(backing_trimmed);

                // Ensure it's a LUKS image
                if crate::services::luks::is_luks_image(&backing_path_buf) {
                    return backing_path_buf
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

pub fn get_system_mounts() -> Vec<(String, PathBuf)> {
    let mut mounts = Vec::new();
    let home_dir = dirs::home_dir().unwrap_or_default();

    if let Ok(content) = fs::read_to_string("/proc/self/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() >= 3 {
                let dev_node = parts[0]; // e.g., /dev/dm-1 or /dev/mapper/luks-...
                let path_str = parts[1]; // e.g., /run/media/neo/b41cf1d7-...
                let fs_type = parts[2];
                let path = PathBuf::from(path_str);

                let is_external = path_str.starts_with("/run/media/")
                    || path_str.starts_with("/media/")
                    || path_str.starts_with("/mnt/");

                let is_user_fuse = fs_type.contains("fuse") && path.starts_with(&home_dir);

                if is_external || is_user_fuse {
                    if path == home_dir {
                        continue;
                    }

                    // Check if this mount originates from a LUKS loop backing file
                    let mut display_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    if let Some(luks_name) = resolve_luks_loop_name(dev_node) {
                        display_name = luks_name;
                    }

                    if !mounts.iter().any(|(_, p)| p == &path) {
                        mounts.push((display_name, path));
                    }
                }
            }
        }
    }

    // Append active network mounts directly into system mounts
    for (uri, name, _icon) in crate::services::network::active_mounts() {
        let path = PathBuf::from(uri);
        if path.is_absolute() && !mounts.iter().any(|(_, p)| p == &path) {
            mounts.push((name, path));
        }
    }

    mounts
}
