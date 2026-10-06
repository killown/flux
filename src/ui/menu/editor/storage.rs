use crate::model::MenuEntry;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>, bool)> {
    let input = input.trim();

    let remainder = input.strip_prefix('"')?;
    let (mime, rest) = remainder.split_once('"')?;

    let second_part = rest.trim().strip_prefix(',')?.trim();

    let cmd_inner = second_part.strip_prefix('"')?;
    let (cmd, after_cmd) = cmd_inner.split_once('"')?;

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

fn flux_base_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux")
}

pub(super) fn get_available_menus() -> Vec<String> {
    let mut menus = vec!["menu.rs".to_string()];
    let base = flux_base_dir();

    // Check root config dir
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("menu") && name.ends_with(".rs") && name != "menu.rs" {
                        menus.push(name.to_string());
                    }
                }
            }
        }
    }

    // Check menus/ subdirectory
    let sub = base.join("menus");
    if let Ok(entries) = fs::read_dir(sub) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("menu")
                        && name.ends_with(".rs")
                        && !menus.contains(&name.to_string())
                    {
                        menus.push(name.to_string());
                    }
                }
            }
        }
    }

    menus.sort();
    menus
}

pub(super) fn config_path_for(menu_name: &str) -> PathBuf {
    let base = flux_base_dir();
    let sub_path = base.join("menus").join(menu_name);
    if sub_path.exists() {
        sub_path
    } else {
        let root_path = base.join(menu_name);
        if root_path.exists() || menu_name == "menu.rs" {
            root_path
        } else {
            base.join("menus").join(menu_name)
        }
    }
}

pub(super) fn load_from_disk(menu_name: &str) -> Vec<MenuEntry> {
    let content = fs::read_to_string(config_path_for(menu_name)).unwrap_or_default();
    let mut entries = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some((left, right)) = line.split_once("=>") else {
            continue;
        };
        let full_label = left.trim().trim_matches('"');
        let (submenu, label) = match full_label.split_once(" > ") {
            Some((s, l)) => (Some(s.to_string()), l.to_string()),
            None => (None, full_label.to_string()),
        };
        let Some((mime, cmd, toast, no_command_dialog)) = split_mime_cmd(right) else {
            continue;
        };
        entries.push(MenuEntry {
            label,
            submenu,
            mime_types: mime,
            command: cmd,
            toast,
            no_command_dialog,
        });
    }
    entries
}

pub(super) fn write_to_disk(menu_name: &str, entries: &[MenuEntry]) -> std::io::Result<()> {
    let path = config_path_for(menu_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&path)?;
    for entry in entries {
        writeln!(file, "{}", entry.to_config_line())?;
    }
    Ok(())
}
