use crate::ui::constants;
use crate::utils::PathExt;
use adw::gdk;
use adw::prelude::*;
use gtk::gdk_pixbuf;
use gtk::gio;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::CustomAction;

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
"󰆏      Copy" => "all", "builtin::copy", "Copied to clipboard"
"󰆐      Cut" => "all", "builtin::cut", "Cut to clipboard"
"󰏊      Paste" => "all", "builtin::paste", "Pasted items"
"󰱝      Open With..." => "file", "builtin::open_with"
"󰩹      Move to Trash" => "all", "gio trash %p", "Moved to trash"
"󰦬      Restore File" => "trash", "gio trash --restore %p", "File restored"
"󰆴      Shred File (Permanent)" => "all", "python $HOME/.local/share/flux/scripts/flux_shredder.py %p", "Shredder initialized"

# --- Navigation & System ---
"      Open Terminal" => "directory", "alacritty --working-directory=%p"
"󰨞      Open in VSCode" => "text/all, application/all", "code %p"
"󰋽      File Properties" => "file", "flux-fm --file-properties %p"
"󰋊      Folder Info" => "directory", "baobab %p"
"󰉋      New Folder" => "directory", "mkdir %p/New-Folder", "Folder created"

# --- Media Edit ---
"󰽰      Media Edit > Join Videos" => "video/all", "python3 $HOME/.local/share/flux/scripts/join_videos.py %p", "Joining videos..."
"󰽰      Media Edit > Cut Video" => "video/all", "python $HOME/.local/share/flux/scripts/video_cutter.py %p", "Opening Video Cutter..."
"󰽰      Media Edit > Mix Audio" => "audio/", "python3 $HOME/.local/share/flux/scripts/mix_audio.py %p", "Mixing audio..."
"󰽰      Media Edit > Merge Video + Audio" => "video/all+audio/all", "python $HOME/.local/share/flux/scripts/join_mp4_mp3.py %p", "Merging media..."

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

# --- Tools ---
"󰯦      Tools > Git Gui" => "directory", "git gui"
"󰯦      Tools > Download Video (1080p)" => "directory", "cd %p && yt-dlp -f 'bv[height<=1080]+ba/b[height<=1080]' $(wl-paste)", "Video download started"
"󰯦      Tools > Copy Path" => "all", "echo -n %p | wl-copy", "Path copied to clipboard"
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
        let _ = fs::write(config_path, toml_str);
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

    let mut config: crate::model::Config = fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_else(|| crate::model::Config {
            ui: crate::model::UIConfig {
                default_icon_size: 128,
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
            },
            sidebar: vec![],
            shortcuts: crate::model::ShortcutsConfig::default(),
        });

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
        crate::utils::save_config(&config);
    }

    config
}

/// Parses the right-hand side of a menu config line into (mime, command, optional_toast).
fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>)> {
    let input = input.trim();

    let remainder = input.strip_prefix('"')?;
    let (mime, rest) = remainder.split_once('"')?;

    let second_part = rest.trim().strip_prefix(',')?.trim();
    // Find the closing quote of the command, allowing for escaped content
    let cmd_inner = second_part.strip_prefix('"')?;
    let (cmd, after_cmd) = cmd_inner.split_once('"')?;

    // Optional 3rd field: , "toast message"
    let toast = after_cmd
        .trim()
        .strip_prefix(',')
        .and_then(|s| s.trim().strip_prefix('"'))
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| s.to_string());

    Some((mime.to_string(), cmd.to_string(), toast))
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

            if let Some((mimes_part, cmd_part, toast)) = split_mime_cmd(right) {
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
                });
            }
        }
    }
    actions
}

pub fn get_icon_for_path(path: &Path, is_dir: bool) -> adw::gio::Icon {
    if is_dir {
        return gio::Icon::for_string("folder").unwrap();
    }
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
    adw::gio::content_type_get_icon(&content_type)
}

pub fn get_mime_type(path: &Path) -> String {
    if path.is_dir() {
        return "inode/directory".to_string();
    }

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let mut sniff_buffer = [0u8; 4096];

    let data_slice = if let Ok(mut file) = fs::File::open(path) {
        if let Ok(count) = file.read(&mut sniff_buffer) {
            &sniff_buffer[..count]
        } else {
            &[]
        }
    } else {
        &[]
    };

    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), data_slice);
    content_type.to_string()
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
    let file = adw::gio::File::for_path(&path);
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
    if let Some(app_info) = adw::gio::AppInfo::default_for_type(&content_type, false) {
        let _ = app_info.launch(&[file], None::<&adw::gio::AppLaunchContext>);
    } else {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}

pub fn get_or_create_thumbnail(path: &Path) -> Option<gdk::Texture> {
    let cache_dir = dirs::cache_dir()?.join("flux").join("thumbnails");
    if fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);

    if let Ok(metadata) = path.metadata() {
        metadata.len().hash(&mut hasher);
        if let Ok(modified) = metadata.modified() {
            modified.hash(&mut hasher);
        }
    }

    let hash = hasher.finish();
    let cache_path = cache_dir.join(format!("{}.png", hash));

    if cache_path.exists() {
        let file = adw::gio::File::for_path(&cache_path);
        return gdk::Texture::from_file(&file).ok();
    }

    let (is_img, is_vid) = is_visual_media(path);
    if is_img {
        match gdk_pixbuf::Pixbuf::from_file_at_scale(
            path,
            constants::CACHED_THUMBNAIL_SIZE,
            constants::CACHED_THUMBNAIL_SIZE,
            true,
        ) {
            Ok(pixbuf) => {
                if let Some(path_str) = cache_path.to_str() {
                    let _ = pixbuf.savev(path_str, "png", &[]);
                }
                return Some(gdk::Texture::for_pixbuf(&pixbuf));
            }
            Err(_) => return None,
        }
    } else if is_vid {
        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-loglevel")
            .arg("panic")
            .arg("-i")
            .arg(path)
            .arg("-ss")
            .arg("00:00:01.000")
            .arg("-vframes")
            .arg("1")
            .arg("-vf")
            .arg("scale=512:-1")
            .arg(&cache_path)
            .status();
        if let Ok(s) = status {
            if s.success() && cache_path.exists() {
                let file = adw::gio::File::for_path(&cache_path);
                return gdk::Texture::from_file(&file).ok();
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
                let path_str = parts[1];
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

                    if let Some(name) = path.file_name() {
                        let display_name = name.to_string_lossy().to_string();
                        if !mounts.iter().any(|(_, p)| p == &path) {
                            mounts.push((display_name, path));
                        }
                    }
                }
            }
        }
    }
    mounts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_ensure_config_file_creation() {
        let temp_dir = env::current_dir()
            .unwrap()
            .join("target")
            .join("test_config_init");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).unwrap();
        }
        fs::create_dir_all(&temp_dir).unwrap();

        env::set_var("XDG_CONFIG_HOME", &temp_dir);

        let path = ensure_config_file();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("flux"));

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_get_system_mounts_structure() {
        let mounts = get_system_mounts();
        assert!(!mounts.is_empty());

        for (name, path) in mounts {
            assert!(!name.is_empty(), "Mount name should not be empty");
            assert!(path.is_absolute(), "Mount path must be absolute");
        }
    }

    #[test]
    fn test_config_invalid_toml() {
        let invalid_toml = "invalid = [unclosed bracket";
        let result: Result<crate::model::Config, _> = toml::from_str(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_missing_fields() {
        let partial_toml = r#"
            [ui]
            sidebar_width = 300
        "#;

        let config: crate::model::Config = toml::from_str(partial_toml).unwrap_or_default();

        assert_eq!(config.ui.sidebar_width, 300);
        assert_eq!(config.ui.default_icon_size, 0);
    }

    /// Verifies that the menu parser can handle the default internal config string.
    #[test]
    fn test_load_menu_config_integration() {
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");

        // Use tempfile to avoid manual cleanup and path collisions
        let tmp = tempfile::TempDir::new().unwrap();
        let temp_dir = tmp.path();

        let flux_config_dir = temp_dir.join("flux");
        std::fs::create_dir_all(&flux_config_dir).unwrap();

        // Set environment for the current process
        std::env::set_var("XDG_CONFIG_HOME", temp_dir);

        let config_path = flux_config_dir.join("menu.rs");
        let mock_content =
            r#""<U+F018F>      Copy" => "all", "builtin::copy", "Copied to clipboard""#;
        std::fs::write(config_path, mock_content).unwrap();

        let actions = load_menu_config();

        // Ensure state is restored regardless of assertion outcome
        if let Some(val) = original_xdg {
            std::env::set_var("XDG_CONFIG_HOME", val);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.label.contains("Copy")));
    }
}
