use super::mime::{sanitise_for_filename, split_mime};
use crate::domain::model::CustomAction;
use crate::i18n::tr;
use crate::utils::config::split_mime_cmd;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn resolve_secondary_menu_template(mime: &str) -> Option<PathBuf> {
    crate::hit!("resolve_secondary_menu_template");
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
    crate::hit!("parse_secondary_template");
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

/// One action to append to a secondary template, as typed into the "New Action" dialog.
#[derive(Debug, Clone)]
pub struct NewSecondaryAction {
    pub label: String,
    pub submenu: Option<String>,
    pub mime_types: String,
    pub command: String,
    pub toast: Option<String>,
    pub no_command_dialog: bool,
}

/// MIME pattern covering an entire category, as accepted by `action_matches_mime`.
pub fn category_mime_pattern(category: &str) -> String {
    match category {
        "" => "all".to_string(),
        "image" | "video" | "audio" | "font" | "application" | "text" => {
            format!("{category}/all")
        }
        other => format!("{other}/"),
    }
}

/// File name of the template that `resolve_secondary_menu_template` consults for `mime`.
pub fn secondary_template_filename(mime: &str, category_scope: bool) -> String {
    let (category, subtype) = split_mime(mime);
    let safe_cat = sanitise_for_filename(category);
    let safe_sub = sanitise_for_filename(subtype);
    if safe_cat.is_empty() {
        "menu-all.rs".to_string()
    } else if category_scope || safe_sub.is_empty() || safe_sub == "all" {
        format!("menu-{safe_cat}-all.rs")
    } else {
        format!("menu-{safe_cat}-{safe_sub}.rs")
    }
}

/// Rejects values the line format can't represent (quotes, newlines).
fn check_field(name: &str, value: &str) -> Result<(), String> {
    if value.contains(['"', '\n', '\r']) {
        return Err(format!(
            "{}: {}",
            name,
            tr("quotes and line breaks are not allowed")
        ));
    }
    Ok(())
}

/// Renders `action` as one `menu-*.rs` line, without the trailing newline.
fn format_secondary_line(action: &NewSecondaryAction) -> Result<String, String> {
    let label = action.label.trim();
    let submenu = action
        .submenu
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let mimes = action.mime_types.trim();
    let command = action.command.trim();
    let toast = action
        .toast
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    if label.is_empty() {
        return Err(tr("The label cannot be empty"));
    }
    if mimes.is_empty() {
        return Err(tr("The MIME types cannot be empty"));
    }
    if command.is_empty() {
        return Err(tr("The command cannot be empty"));
    }
    check_field(&tr("Label"), label)?;
    if let Some(sub) = submenu {
        check_field(&tr("Submenu"), sub)?;
    }
    check_field(&tr("MIME Types"), mimes)?;
    check_field(&tr("Command"), command)?;
    if let Some(t) = toast {
        check_field(&tr("Notification"), t)?;
    }
    if label.contains("=>") || submenu.is_some_and(|s| s.contains("=>")) {
        return Err(tr("The label cannot contain \"=>\""));
    }

    let full_label = match submenu {
        Some(sub) => format!("{sub} > {label}"),
        None => label.to_string(),
    };
    let mut line = format!("\"{full_label}\" => \"{mimes}\", \"{command}\"");
    if let Some(t) = toast {
        line.push_str(&format!(", \"{t}\""));
    }
    if action.no_command_dialog {
        line.push_str(", \"no_command_dialog\"");
    }
    Ok(line)
}

/// Appends `action` to the template file for `mime`, creating it if missing.
///
/// Existing files are used wherever they live (`~/.config/flux/menus/`, then the
/// legacy `~/.config/flux/`), new files are created in `menus/`.
pub fn append_secondary_action(
    mime: &str,
    category_scope: bool,
    action: &NewSecondaryAction,
) -> Result<PathBuf, String> {
    crate::hit!("append_secondary_action");
    let line = format_secondary_line(action)?;

    let base_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("flux");
    let menus_dir = base_dir.join("menus");
    let filename = secondary_template_filename(mime, category_scope);

    let existing = [&menus_dir, &base_dir]
        .into_iter()
        .map(|d| d.join(&filename))
        .find(|p| p.is_file());
    let is_new = existing.is_none();
    let target = existing.unwrap_or_else(|| menus_dir.join(&filename));

    let mut out = String::new();
    if is_new {
        std::fs::create_dir_all(&menus_dir).map_err(|e| e.to_string())?;
        out.push_str(&format!("// Flux contextual actions ({filename})\n"));
    } else {
        let current = std::fs::read_to_string(&target).map_err(|e| e.to_string())?;
        if !current.is_empty() && !current.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str(&line);
    out.push('\n');

    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&target)
        .and_then(|mut f| f.write_all(out.as_bytes()))
        .map_err(|e| e.to_string())?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action() -> NewSecondaryAction {
        NewSecondaryAction {
            label: "Optimise".into(),
            submenu: Some("Image".into()),
            mime_types: "image/png".into(),
            command: "oxipng %p".into(),
            toast: None,
            no_command_dialog: false,
        }
    }

    #[test]
    fn line_round_trips_through_the_parser() {
        let mut a = action();
        a.toast = Some("Done".into());
        a.no_command_dialog = true;
        let line = format_secondary_line(&a).unwrap();
        let (left, right) = line.split_once("=>").unwrap();
        assert_eq!(left.trim(), "\"Image > Optimise\"");
        let (mime, cmd, toast, no_dialog) = split_mime_cmd(right).unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(cmd, "oxipng %p");
        assert_eq!(toast.as_deref(), Some("Done"));
        assert!(no_dialog);
    }

    #[test]
    fn rejects_unrepresentable_input() {
        let mut a = action();
        a.command = "echo \"hi\"".into();
        assert!(format_secondary_line(&a).is_err());
        let mut a = action();
        a.label = "  ".into();
        assert!(format_secondary_line(&a).is_err());
    }

    #[test]
    fn filenames_follow_the_resolver() {
        assert_eq!(
            secondary_template_filename("image/png", false),
            "menu-image-png.rs"
        );
        assert_eq!(
            secondary_template_filename("image/png", true),
            "menu-image-all.rs"
        );
        assert_eq!(
            secondary_template_filename("text/", false),
            "menu-text-all.rs"
        );
        assert_eq!(secondary_template_filename("", false), "menu-all.rs");
    }
}
