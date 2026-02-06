use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use adw::prelude::*;
use adw::gdk;
use gtk::gdk_pixbuf;
use gtk::gio;
use std::env;

pub fn ensure_config_file() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("flux");
    if !config_dir.exists() { let _ = fs::create_dir_all(&config_dir); }
    let config_path = config_dir.join("menu.rs");
    if !config_path.exists() {
        let default_config = r#""Open Terminal" => "alacritty --working-directory=%d"
"Copy Path" => "echo -n %p | wl-copy"
"Move to Trash" => "gio trash %p"
"Open in Code" => "code %p""#;
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

[ui.folder_sort]
# "/home/neo/Downloads" = "Date"

[ui.device_renames]

[[sidebar]]
name = "Projects"
icon = "folder-saved-search-symbolic"
path = "~/Projects"
"#;
        let _ = fs::write(&config_path, default_toml);
    }

    fs::read_to_string(config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_else(|| crate::model::Config {
            ui: crate::model::UIConfig {
                default_icon_size: 128,
                sidebar_width: 240,
                show_xdg_dirs: true,
                default_sort: crate::model::SortBy::Name,
                folder_sort: std::collections::HashMap::new(),
                show_hidden_by_default: false,
                show_xdg_dirs_by_default: true,
                device_renames: std::collections::HashMap::new(),
            },
            sidebar: vec![],
        })
}

pub fn load_menu_config() -> (gio::Menu, Vec<(String, String)>) {
    let path = ensure_config_file();
    let menu = gio::Menu::new();
    let mut actions = Vec::new();
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("//") || line.is_empty() { continue; }
            if let Some((label_part, cmd_part)) = line.split_once("=>") {
                let label = label_part.trim().trim_matches('"').trim();
                let cmd = cmd_part.trim().trim_matches(',').trim().trim_matches('"');
                if !label.is_empty() && !cmd.is_empty() {
                    let action_name = label.to_lowercase().replace(" ", "_");
                    let full_action_name = format!("win.{}", action_name);
                    menu.append(Some(label), Some(&full_action_name));
                    actions.push((action_name, cmd.to_string()));
                }
            }
        }
    }
    (menu, actions)
}

pub fn get_icon_for_path(path: &Path, is_dir: bool) -> adw::gio::Icon {
    if is_dir {
        return gio::Icon::for_string("folder").unwrap(); 
    }
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
    adw::gio::content_type_get_icon(&content_type)
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
        match gdk_pixbuf::Pixbuf::from_file_at_scale(path, 256, 256, true) {
            Ok(pixbuf) => {
                if let Some(path_str) = cache_path.to_str() { let _ = pixbuf.savev(path_str, "png", &[]); }
                return Some(gdk::Texture::for_pixbuf(&pixbuf));
            },
            Err(_) => return None
        }
    } else if is_vid {
        let status = Command::new("ffmpeg").arg("-y").arg("-loglevel").arg("panic").arg("-i").arg(path)
            .arg("-ss").arg("00:00:01.000").arg("-vframes").arg("1").arg("-vf").arg("scale=256:-1").arg(&cache_path).status();
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
    let final_cmd = command_template
        .replace("%p", &format!("\"{}\"", path_str))
        .replace("%d", &format!("\"{}\"", parent))
        .replace("%f", &format!("\"{}\"", filename));
    let _ = Command::new("sh").arg("-c").arg(final_cmd).spawn();
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
