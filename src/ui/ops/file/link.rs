use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::{Path, PathBuf};

impl FluxApp {
    /// Creates symlinks or hardlinks in the current directory from clipboard paths.
    /// `hard` = true for hardlink, false for symlink.
    pub fn handle_create_link(&self, hard: bool, sender: &AsyncComponentSender<Self>) {
        let Some(display) = gtk::gdk::Display::default() else {
            sender.input(AppMsg::ShowToast(crate::i18n::tr(
                "No clipboard available.",
            )));
            return;
        };

        let clipboard = display.clipboard();
        let s = sender.clone();
        let current_dir = self.current_path.clone();

        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| {
            let text: String = match res {
                Ok(Some(t)) => t.to_string(),
                _ => {
                    s.input(AppMsg::ShowToast(crate::i18n::tr(
                        "Clipboard does not contain text.",
                    )));
                    return;
                }
            };

            let mut targets = Vec::new();
            let mut current_token = String::new();
            let mut in_quote: Option<char> = None;
            let mut chars = text.chars().peekable();

            while let Some(c) = chars.next() {
                if let Some(q) = in_quote {
                    if c == q {
                        if q == '\'' && chars.peek() == Some(&'\\') {
                            chars.next();
                            if chars.peek() == Some(&'\'') {
                                chars.next();
                                current_token.push('\'');
                                continue;
                            }
                        }
                        in_quote = None;
                    } else {
                        current_token.push(c);
                    }
                } else {
                    match c {
                        '\'' | '"' => in_quote = Some(c),
                        ' ' | '\t' | '\r' | '\n' => {
                            if !current_token.is_empty() {
                                let trimmed = current_token.trim();
                                let stripped = trimmed
                                    .strip_prefix("file://")
                                    .unwrap_or(trimmed)
                                    .strip_prefix("FILE://")
                                    .unwrap_or(trimmed);
                                let p = PathBuf::from(stripped);
                                if !p.as_os_str().is_empty() {
                                    targets.push(p);
                                }
                                current_token.clear();
                            }
                        }
                        _ => current_token.push(c),
                    }
                }
            }
            if !current_token.is_empty() {
                let trimmed = current_token.trim();
                let stripped = trimmed
                    .strip_prefix("file://")
                    .unwrap_or(trimmed)
                    .strip_prefix("FILE://")
                    .unwrap_or(trimmed);
                let p = PathBuf::from(stripped);
                if !p.as_os_str().is_empty() {
                    targets.push(p);
                }
            }

            if targets.is_empty() {
                s.input(AppMsg::ShowToast(crate::i18n::tr(
                    "No valid paths found in clipboard.",
                )));
                return;
            }

            relm4::spawn_blocking(move || {
                let mut created = Vec::new();
                let mut errors = Vec::new();

                // Canonicalize the current directory once
                let current_canon = match current_dir.canonicalize() {
                    Ok(c) => c,
                    Err(e) => {
                        s.input(AppMsg::ShowToast(format!(
                            "Cannot resolve current directory: {}",
                            e
                        )));
                        return;
                    }
                };

                for target in targets {
                    // Resolve absolute path
                    let target_abs = if target.is_absolute() {
                        target
                    } else {
                        current_dir.join(&target)
                    };

                    // Canonicalize to resolve symlinks
                    let target_canon = match target_abs.canonicalize() {
                        Ok(c) => c,
                        Err(e) => {
                            errors.push(format!("{}: {}", target_abs.display(), e));
                            continue;
                        }
                    };

                    if !target_canon.exists() {
                        errors.push(format!(
                            "{}: {}",
                            target_canon.display(),
                            crate::i18n::tr("No such file or directory")
                        ));
                        continue;
                    }

                    let basename = target_canon
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if basename.is_empty() {
                        errors.push(format!(
                            "{}: {}",
                            target_canon.display(),
                            crate::i18n::tr("Invalid filename")
                        ));
                        continue;
                    }

                    // Generate a unique link name in the current directory
                    let mut link_name = basename.clone();
                    let mut counter = 1;
                    while current_dir.join(&link_name).exists() {
                        if let Some(stem) =
                            Path::new(&basename).file_stem().and_then(|s| s.to_str())
                        {
                            let ext = Path::new(&basename)
                                .extension()
                                .and_then(|s| s.to_str())
                                .map(|e| format!(".{}", e))
                                .unwrap_or_default();
                            link_name = format!("{} ({}){}", stem, counter, ext);
                        } else {
                            link_name = format!("{}_{}", basename, counter);
                        }
                        counter += 1;
                    }
                    let link_path = current_dir.join(&link_name);

                    // Create the link
                    let result = if hard {
                        if target_canon.is_dir() {
                            errors.push(format!(
                                "{}: {}",
                                target_canon.display(),
                                crate::i18n::tr("Hard links to directories are not supported.")
                            ));
                            continue;
                        }
                        std::fs::hard_link(&target_canon, &link_path)
                    } else {
                        // Compute relative path from canonical current dir to canonical target
                        let rel = pathdiff::diff_paths(&target_canon, &current_canon)
                            .unwrap_or_else(|| target_canon.clone());
                        std::os::unix::fs::symlink(&rel, &link_path)
                    };

                    match result {
                        Ok(()) => created.push(link_path),
                        Err(e) => errors.push(format!("{}: {}", link_path.display(), e)),
                    }
                }

                if !created.is_empty() {
                    let msg = if created.len() == 1 {
                        format!(
                            "{}: {}",
                            crate::i18n::tr("Created link"),
                            created[0].display()
                        )
                    } else {
                        format!("{} {} links", crate::i18n::tr("Created"), created.len())
                    };
                    s.input(AppMsg::ShowToast(msg));
                    s.input(AppMsg::Refresh);
                }
                if !errors.is_empty() {
                    s.input(AppMsg::ShowToast(format!(
                        "{}: {}",
                        crate::i18n::tr("Errors"),
                        errors.join("; ")
                    )));
                }
            });
        });
    }
}
