//! Process spawning, editor launch and resource registration.

use super::shell::{expand_template, shell_safe, Placeholder};
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
    crate::hit!("open_new_instance");
    if !path.is_dir() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    std::process::Command::new(exe).arg(path).spawn().is_ok()
}

/// Builds the `sh -c` command for an editor configured with `%p` / `%l`.
///
/// The path is single-quoted like every other shell argument, and the
/// template is expanded in one pass so the path is never scanned for
/// placeholders. Quotes written around `%p` in the config (`"%p"`) are
/// absorbed. Returns `None` when `editor` uses neither placeholder, or when
/// the path cannot be quoted safely (newline, carriage return or NUL).
pub fn editor_shell_command(editor: &str, file_path: &Path, line: usize) -> Option<String> {
    if !(editor.contains("%p") || editor.contains("%l")) {
        return None;
    }
    let path_arg = shell_safe(&file_path.to_string_lossy())?;
    expand_template(editor, |token: &str| match token {
        "%p" => Placeholder::Shell(path_arg.clone()),
        "%l" => Placeholder::Plain(line.to_string()),
        _ => Placeholder::Unknown,
    })
}

/// Spawns the user-configured editor detached without task queue overhead or progress dialogs.
pub fn launch_editor_at_line(
    editor_cfg: &str,
    file_path: &Path,
    line: usize,
    query: Option<&str>,
) -> std::io::Result<()> {
    crate::hit!("launch_editor_at_line");
    let editor = if editor_cfg.trim().is_empty() {
        "nvim"
    } else {
        editor_cfg.trim()
    };

    let target = file_path.to_string_lossy().to_string();

    if editor.contains("%p") || editor.contains("%l") {
        let Some(cmd_str) = editor_shell_command(editor, file_path, line) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "file name contains characters that cannot be passed to a shell",
            ));
        };
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
    crate::hit!("register_resources");
    {
        crate::hit!("register_resources:gresource");
        gio::resources_register_include!("flux.gresource").expect("failed to register resources");
    }

    if let Some(display) = gtk::gdk::Display::default() {
        crate::hit!("register_resources:icon_theme");
        gtk::IconTheme::for_display(&display).add_resource_path("/io/github/killown/flux/icons");
    }
}
