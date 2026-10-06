use std::fs;
use std::path::PathBuf;

use super::object::NerdIconObject;

fn resolve_nerd_fonts_file() -> Option<PathBuf> {
    if let Some(user_data) = dirs::data_dir().map(|d| d.join("flux/nerd_fonts.json")) {
        if user_data.exists() {
            return Some(user_data);
        }
    }

    if let Some(user_cfg) = dirs::config_dir().map(|d| d.join("flux/nerd_fonts.json")) {
        if user_cfg.exists() {
            return Some(user_cfg);
        }
    }

    let flatpak_path = PathBuf::from("/app/share/flux/nerd_fonts.json");
    if flatpak_path.exists() {
        return Some(flatpak_path);
    }

    let sys_path = PathBuf::from("/usr/share/flux/nerd_fonts.json");
    if sys_path.exists() {
        return Some(sys_path);
    }

    let local_dev = PathBuf::from("assets/nerd_fonts.json");
    if local_dev.exists() {
        return Some(local_dev);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("assets/nerd_fonts.json");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

fn extract_json_strings(content: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();

    for c in content.chars() {
        if in_string {
            if escaped {
                match c {
                    '"' => current.push('"'),
                    '\\' => current.push('\\'),
                    'n' => current.push('\n'),
                    'r' => current.push('\r'),
                    't' => current.push('\t'),
                    'u' => {
                        // Keep escapes as-is or literal
                        current.push('\\');
                        current.push('u');
                    }
                    other => {
                        current.push('\\');
                        current.push(other);
                    }
                }
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                tokens.push(std::mem::take(&mut current));
                in_string = false;
            } else {
                current.push(c);
            }
        } else if c == '"' {
            in_string = true;
        }
    }
    tokens
}

pub(super) fn load_installed_json_entries() -> Vec<NerdIconObject> {
    let Some(path) = resolve_nerd_fonts_file() else {
        eprintln!("[flux] nerd_fonts.json not found in ~/.local/share/flux/, /usr/share/flux/, or ./assets/");
        return Vec::new();
    };

    let Ok(content) = fs::read_to_string(&path) else {
        eprintln!("[flux] Failed to read nerd_fonts.json at {:?}", path);
        return Vec::new();
    };

    let tokens = extract_json_strings(&content);
    let mut items = Vec::new();

    let mut i = 0;
    while i + 1 < tokens.len() {
        let t1 = &tokens[i];
        let t2 = &tokens[i + 1];

        if t2 == "char" && i + 2 < tokens.len() {
            let t3 = &tokens[i + 2];
            items.push(NerdIconObject::new(t3, t1));
            i += 3;
            continue;
        }

        let t1_is_glyph = t1.chars().count() == 1;
        let t2_is_glyph = t2.chars().count() == 1;

        if t1_is_glyph && !t2_is_glyph {
            items.push(NerdIconObject::new(t1, t2));
            i += 2;
        } else if t2_is_glyph && !t1_is_glyph {
            items.push(NerdIconObject::new(t2, t1));
            i += 2;
        } else {
            i += 1;
        }
    }

    eprintln!(
        "[flux] Loaded {} Nerd Font icons from {:?}",
        items.len(),
        path
    );
    items
}
