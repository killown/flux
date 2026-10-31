//! Layering rules for the source tree.
//!
//! `tests/domain/layering.rs` keeps `domain` free of other layers and toolkits.
//! This target covers the layers above it: `utils` and `infra` sit below `app` and
//! `ui`, so they must not import them, and `SENDER` is reached through `infra`.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The code part of a line: comments and doc comments may name other layers.
fn code_part(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

/// Matches `crate::app`, `crate::ui` and `crate::cli` paths, including the first
/// segment of a one-line `crate::{...}` import list.
fn upward() -> Regex {
    Regex::new(r"\bcrate::(?:(?:app|ui|cli)\b|\{[^}]*\b(?:app|ui|cli)\b)").unwrap()
}

/// Matches the old `app::state::SENDER` path.
fn old_sender_path() -> Regex {
    Regex::new(r"\bapp::state::SENDER\b").unwrap()
}

/// `file:line: text` for every code line under `src/<layer>` that matches `pattern`.
/// An empty `layer` scans the whole of `src/`.
fn violations(layer: &str, pattern: &Regex) -> Vec<String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src.join(layer), &mut files);
    assert!(!files.is_empty(), "no sources found under src/{layer}");
    files.sort();
    let mut found = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if pattern.is_match(code_part(line)) {
                let rel = file.strip_prefix(&src).unwrap_or(&file).display();
                found.push(format!("src/{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    found
}

#[test]
fn utils_does_not_depend_on_app_ui_or_cli() {
    let found = violations("utils", &upward());
    assert!(
        found.is_empty(),
        "utils sits below app and ui; move the shared item down instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn infra_does_not_depend_on_app_ui_or_cli() {
    let found = violations("infra", &upward());
    assert!(
        found.is_empty(),
        "infra sits below app and ui; move the shared item down instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn sender_is_reached_through_infra() {
    let found = violations("", &old_sender_path());
    assert!(
        found.is_empty(),
        "use crate::infra::SENDER, not app::state::SENDER:\n{}",
        found.join("\n")
    );
}

#[test]
fn layering_patterns_flag_violations_and_ignore_comments() {
    let up = upward();
    assert!(up.is_match("use crate::ui::constants;"));
    assert!(up.is_match("use crate::app::state::FluxApp;"));
    assert!(up.is_match("use crate::{app::state::FluxApp, utils};"));
    assert!(up.is_match("let w = crate::cli::run();"));
    assert!(!up.is_match("use crate::utils::helpers;"));
    assert!(!up.is_match("use crate::infra::SENDER;"));
    assert!(!up.is_match("use crate::domain::model::constants;"));
    assert!(!up.is_match("use crate::ui_like::X;"));
    assert!(!up.is_match(code_part("let x = 1; // crate::ui::foo")));
    assert!(!up.is_match(code_part("/// Named in docs: crate::app::state")));

    let sender = old_sender_path();
    assert!(sender.is_match("crate::app::state::SENDER.get()"));
    assert!(!sender.is_match("crate::infra::SENDER.get()"));
    assert!(!sender.is_match(code_part("// was app::state::SENDER")));
}
