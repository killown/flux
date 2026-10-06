//! Process spawning, editor launch and resource registration.

use gtk::gio;
use relm4::prelude::*;
use std::path::Path;

/// Spawns a new application instance rooted at `path`.
///
/// Uses the running executable path rather than a hardcoded binary name so
/// dev builds (`flux-fm`) and installed builds (`flux`) both work correctly.
///
/// Returns `false` if the path is not a directory or the exe path cannot be
/// resolved, `true` if the child process was spawned successfully.
pub fn open_new_instance(path: &std::path::Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    std::process::Command::new(exe).arg(path).spawn().is_ok()
}

/// Spawns the user-configured editor detached without task queue overhead or progress dialogs.
pub fn launch_editor_at_line(
    editor_cfg: &str,
    file_path: &Path,
    line: usize,
    query: Option<&str>,
) -> std::io::Result<()> {
    let editor = if editor_cfg.trim().is_empty() {
        "nvim"
    } else {
        editor_cfg.trim()
    };

    let target = file_path.to_string_lossy().to_string();

    if editor.contains("%p") || editor.contains("%l") {
        let cmd_str = editor
            .replace("%p", &format!("\"{}\"", target))
            .replace("%l", &line.to_string());
        return std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd_str)
            .spawn()
            .map(|_| ());
    }

    if matches!(editor, "nvim" | "vim" | "vi") {
        for term in &["alacritty", "kitty", "foot", "wezterm", "xterm"] {
            let mut cmd = std::process::Command::new(term);
            cmd.arg("-e").arg(editor);

            cmd.arg(format!("+{}", line));

            if let Some(q) = query {
                let trimmed = q.trim();
                if !trimmed.is_empty() {
                    cmd.arg(format!("+/\\V{}", trimmed));
                }
            }

            cmd.arg(&target);

            if cmd.spawn().is_ok() {
                return Ok(());
            }
        }
    }

    match editor {
        "code" | "cursor" => std::process::Command::new(editor)
            .arg("--goto")
            .arg(format!("{}:{}", target, line))
            .spawn()
            .map(|_| ()),
        _ => std::process::Command::new(editor)
            .arg(format!("+{}", line))
            .arg(&target)
            .spawn()
            .map(|_| ()),
    }
}

/// Registers the bundled GResource icons with the default icon theme.
pub fn register_resources() {
    gio::resources_register_include!("flux.gresource").expect("failed to register resources");

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_resource_path("/io/github/killown/flux/icons");
    }
}
