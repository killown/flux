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
        //FIXME: file properties not working without full path
        let default_config = r#"
"      Open Terminal" => "directory", "alacritty --working-directory=%p"
"󰆏      Copy" => "all", "builtin::copy"
"󰆐      Cut" => "all", "builtin::cut"
"󰏊      Paste" => "all", "builtin::paste"
"󰩹      Move to Trash" => "all", "gio trash %p"
"󰦬      Restore File" => "trash", "gio trash --restore %p"
"󰸉      Set as Wallpaper" => "image/all", "swww img %p"
"󰨞      Open in Code" => "text/all, application/all", "code %p"
"󰋽      File Properties" => "file", "~/.local/bin/flux --file-properties %p"
"󰱝      Open With..." => "file", "builtin::open_with"
"󰋊      Folder Info" => "directory", "flatpak run org.gnome.baobab %p"
"🛠      Tools > 󰯦   Copy Path" => "all", "echo -n %p | wl-copy"
"🛠      Tools > 󰊢   Git Gui" => "directory", "git gui"
"      Images > 󰸉   Set Wallpaper" => "image/all", "swww img %p""#;
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

fn split_mime_cmd(input: &str) -> Option<(String, String)> {
    let input = input.trim();

    // Extract first quoted part
    let remainder = input.strip_prefix('"')?;
    let (mime, rest) = remainder.split_once('"')?;

    // Extract second quoted part after the comma
    let second_part = rest.trim().strip_prefix(',')?.trim();
    let cmd = second_part.strip_prefix('"')?.strip_suffix('"')?;

    Some((mime.to_string(), cmd.to_string()))
}

pub fn load_menu_config() -> Vec<CustomAction> {
    // 1. USE ensure_config_file() to get the correct path
    let config_path = ensure_config_file();

    let content = std::fs::read_to_string(config_path).unwrap_or_default();
    let mut actions = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // Parse: "Label" => "mimes", "command"
        if let Some((left, right)) = line.split_once("=>") {
            let full_label = left.trim().trim_matches('"');

            // --- Submenu Parsing ---
            // Detect "Group > Item" syntax
            let (submenu, label) = if full_label.contains(" > ") {
                let parts: Vec<&str> = full_label.splitn(2, " > ").collect();
                (Some(parts[0].to_string()), parts[1].to_string())
            } else {
                (None, full_label.to_string())
            };

            // 2. USE split_mime_cmd() to correctly parse the right side
            if let Some((mimes_part, cmd_part)) = split_mime_cmd(right) {
                let mime_types: Vec<String> = mimes_part
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();

                actions.push(CustomAction {
                    label,
                    submenu, // Field added to CustomAction in model.rs
                    action_name: format!("custom_{}", i),
                    command: cmd_part,
                    mime_types,
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

pub fn run_custom_command(command_template: &str, file_path: &Path) {
    let path_str = file_path.to_string_lossy();
    let parent = file_path.parent().unwrap_or(file_path).to_string_lossy();
    let filename = file_path.file_name().unwrap_or_default().to_string_lossy();

    // Escape variables to prevent shell injection
    let p_arg = format!("'{}'", path_str.replace("'", "'\\''"));
    let d_arg = format!("'{}'", parent.replace("'", "'\\''"));
    let f_arg = format!("'{}'", filename.replace("'", "'\\''"));

    let mut final_cmd = command_template
        .replace("%p", &p_arg)
        .replace("%d", &d_arg)
        .replace("%f", &f_arg);

    // MANUALLY EXPAND ~ and $HOME:
    // This ensures that even if the Desktop environment has a limited PATH,
    // we resolve the user's local bin folder correctly.
    //Resolve tildes in the command template ONLY,
    // or better yet, rely on the shell to handle standard shortcuts.
    if final_cmd.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            final_cmd = final_cmd.replacen("~", &home, 1);
        }
    }

    let _ = Command::new("sh").arg("-c").arg(final_cmd).spawn();
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
