use super::mime::{sanitise_for_filename, split_mime};
use crate::model::CustomAction;
use crate::utils::config::split_mime_cmd;
use std::path::{Path, PathBuf};

pub fn resolve_secondary_menu_template(mime: &str) -> Option<PathBuf> {
    let base_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("flux");
    let sub_dir = base_dir.join("menus");
    let search_dirs = if sub_dir.is_dir() {
        vec![sub_dir, base_dir]
    } else {
        vec![base_dir]
    };

    let (category, subtype) = split_mime(mime);

    let safe_cat = sanitise_for_filename(category);
    let safe_sub = sanitise_for_filename(subtype);

    let mut candidate_filenames: Vec<String> = Vec::with_capacity(3);

    if !safe_sub.is_empty() && safe_sub != "all" {
        candidate_filenames.push(format!("menu-{safe_cat}-{safe_sub}.rs"));
    }

    if !safe_cat.is_empty() {
        candidate_filenames.push(format!("menu-{safe_cat}-all.rs"));
    }

    candidate_filenames.push("menu-all.rs".to_string());

    for filename in candidate_filenames {
        for dir in &search_dirs {
            let candidate = dir.join(&filename);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn parse_secondary_template(path: &Path) -> Vec<CustomAction> {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[SecondaryMenu] cannot read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(['-', ' '], "_");

    let mut actions = Vec::new();

    for (line_no, raw_line) in src.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }

        let Some((left, right)) = line.split_once("=>") else {
            eprintln!(
                "[SecondaryMenu] {:?}:{} – missing `=>`, skipping: {}",
                path,
                line_no + 1,
                line
            );
            continue;
        };

        let full_label = left.trim().trim_matches('"');

        let (submenu, label) = if let Some(pos) = full_label.find(" > ") {
            (
                Some(full_label[..pos].trim().to_string()),
                full_label[pos + 3..].trim().to_string(),
            )
        } else {
            (None, full_label.to_string())
        };

        let Some((mimes_part, cmd_part, toast, no_command_dialog)) = split_mime_cmd(right) else {
            eprintln!(
                "[SecondaryMenu] {:?}:{} – malformed RHS, skipping: {}",
                path,
                line_no + 1,
                line
            );
            continue;
        };

        let mime_types: Vec<String> = mimes_part
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        actions.push(CustomAction {
            label,
            submenu,
            action_name: format!("sec_{}_{}", stem, line_no),
            command: cmd_part,
            mime_types,
            toast,
            no_command_dialog,
        });
    }

    actions
}
