use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use adw::prelude::*;
use adw::gdk;
use gtk::gdk_pixbuf;
use gtk::gio;
use std::env;

use crate::model::CustomAction;

pub fn ensure_config_file() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("flux");
    if !config_dir.exists() { let _ = fs::create_dir_all(&config_dir); }
    let config_path = config_dir.join("menu.rs");
    if !config_path.exists() {
        //FIXME: file properties not working without full path
        let default_config = r#""Open Terminal" => "directory", "alacritty --working-directory=%d"
"Copy Path" => "echo -n %p | wl-copy"
"Move to Trash" => "gio trash %p"
"Restore File" => "trash", "gio trash --restore %p"
"Set as Wallpaper" => "image/all", "swww img %p"
"Open in Code" => "text/all, application/all", "code %p"
"File Properties" => "file", "~/.local/bin/flux --file-properties %p""#;
        if let Ok(mut file) = fs::File::create(&config_path) { let _ = file.write_all(default_config.as_bytes()); }
    }
    config_path
}

pub fn save_config(config: &crate::model::Config) {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("flux");
    let config_path = config_dir.join("config.toml");

    if let Ok(toml_str) = toml::to_string_pretty(config) {
        let _ = fs::write(config_path, toml_str);
    }
}

pub fn load_config() -> crate::model::Config {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("flux");
    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        let _ = fs::create_dir_all(&config_dir);
        let default_toml = r#"[ui]
default_icon_size = 96
sidebar_width = 200
show_xdg_dirs = true
default_sort = "Name"
show_hidden_by_default = false
show_xdg_dirs_by_default = true
folders_first = true
theme = "default"

[ui.folder_sort]

[ui.device_renames]

[[sidebar]]
name = "Downloads"
icon = "folder-download-symbolic"
path = "~/Downloads"
"#;
        let _ = fs::write(&config_path, default_toml);
    }

    let mut config: crate::model::Config = fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_else(|| crate::model::Config {
            ui: crate::model::UIConfig {
                default_icon_size: 128,
                sidebar_width: 240,
                show_xdg_dirs: true,
                default_sort: crate::model::SortBy::Name,
                folder_sort: std::collections::HashMap::new(),
                folder_icon_size: std::collections::HashMap::new(),
                show_hidden_by_default: false,
                show_xdg_dirs_by_default: true,
                device_renames: std::collections::HashMap::new(),
                folders_first: true,
                theme: Some("default".to_string()),
            },
            sidebar: vec![],
        });

    let mut changed = false;

    // Pruning logic for folder_sort
    config.ui.folder_sort.retain(|path_str, _| {
        let path = if path_str.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path_str.trim_start_matches("~/")))
                .unwrap_or_else(|| PathBuf::from(path_str))
        } else {
            PathBuf::from(path_str)
        };

        let exists = path.exists();
        if !exists { changed = true; }
        exists
    });

    // Pruning logic for folder_icon_size
    config.ui.folder_icon_size.retain(|path_str, _| {
        let path = if path_str.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path_str.trim_start_matches("~/")))
                .unwrap_or_else(|| PathBuf::from(path_str))
        } else {
            PathBuf::from(path_str)
        };

        let exists = path.exists();
        if !exists { changed = true; }
        exists
    });

    // Only write to disk if something was actually removed
    if changed {
        crate::utils::save_config(&config);
    }

    config
}

fn split_mime_cmd(input: &str) -> Option<(String, String)> {
    if input.starts_with('"') {
        if let Some(first_end) = input[1..].find('"') {
            let first_end = first_end + 1; 
            let remainder = &input[first_end+1..].trim();
            if remainder.starts_with(',') {
                let second_part = remainder[1..].trim();
                if second_part.starts_with('"') && second_part.ends_with('"') {
                    let mime = input[1..first_end].to_string();
                    let cmd = second_part.trim_matches('"').to_string();
                    return Some((mime, cmd));
                }
            }
        }
    }
    None
}

pub fn load_menu_config() -> Vec<CustomAction> {
    let path = ensure_config_file();
    let mut actions = Vec::new();
 
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("//") || line.is_empty() { continue; }
 
            if let Some((label_part, rest)) = line.split_once("=>") {
                let label = label_part.trim().trim_matches('"').trim();
                let rest = rest.trim();

                let (mime_str, cmd) = if let Some((mime_part, cmd_part)) = split_mime_cmd(rest) {
                    (mime_part, cmd_part)
                } else {
                    ("*".to_string(), rest.trim_matches('"').to_string())
                };

                let mime_types: Vec<String> = mime_str.split(',')
                    .map(|s| s.trim().to_string())
                    .collect();

                if !label.is_empty() && !cmd.is_empty() {
                    let action_name = label.to_lowercase().replace(" ", "_").replace("!", "");
                    actions.push(CustomAction {
                        label: label.to_string(),
                        action_name,
                        command: cmd,
                        mime_types,
                    });
                }
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
    (content_type.starts_with("image/"), content_type.starts_with("video/"))
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
    if let Err(_) = fs::create_dir_all(&cache_dir) { return None; }
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
        match gdk_pixbuf::Pixbuf::from_file_at_scale(path, 512, 512, true) {
            Ok(pixbuf) => {
                if let Some(path_str) = cache_path.to_str() { let _ = pixbuf.savev(path_str, "png", &[]); }
                return Some(gdk::Texture::for_pixbuf(&pixbuf));
            },
            Err(_) => return None
        }
    } else if is_vid {
        let status = Command::new("ffmpeg").arg("-y").arg("-loglevel").arg("panic").arg("-i").arg(path)
            .arg("-ss").arg("00:00:01.000").arg("-vframes").arg("1").arg("-vf").arg("scale=512:-1").arg(&cache_path).status();
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
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        final_cmd = final_cmd.replace("~", &home_str).replace("$HOME", &home_str);
    }

    let _ = Command::new("sh")
        .arg("-c")
        .arg(final_cmd)
        .spawn();
}

pub fn get_xdg_dir(env_var: &str, fallback: &str) -> PathBuf {
    match env::var(env_var) {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            if let Some(home) = dirs::home_dir() {
                PathBuf::from(fallback.replace("~", &home.to_string_lossy()))
            } else {
                PathBuf::from(fallback)
            }
        }
    }
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

                let is_external = path_str.starts_with("/run/media/") || 
                                 path_str.starts_with("/media/") || 
                                 path_str.starts_with("/mnt/");

                let is_user_fuse = fs_type.contains("fuse") && path.starts_with(&home_dir);

                if is_external || is_user_fuse {
                    if path == home_dir { continue; }

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
