//! Enforces the layering rule stated in `src/domain/model/mod.rs`: nothing under
//! `src/domain` may depend on `ui`, `infra`, `app` or a toolkit crate.
//!
//! `domain` holds plain data and pure logic. If this test fails, move the
//! offending type or call out of `domain` (see `app::state` for state that owns
//! widgets, and `utils::media::{image_from_texture, texture_from_image}` for how
//! pixels cross the boundary), do not add the crate to an allow list.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// The GTK stack and Relm4. `tokio::sync` primitives are deliberately not listed:
/// they are runtime-agnostic data channels, not a toolkit.
const TOOLKIT_CRATES: &[&str] = &[
    "gtk",
    "gtk4",
    "gdk",
    "gdk_pixbuf",
    "gio",
    "gio_unix",
    "glib",
    "adw",
    "libadwaita",
    "pango",
    "pangocairo",
    "vte",
    "poppler",
    "relm4",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read src/domain") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Matches `gtk::…` style paths and `crate::{ui,infra,app}` in either the
/// plain (`crate::ui::x`) or grouped (`crate::{ui, x}`) form.
fn forbidden() -> Regex {
    let crates = TOOLKIT_CRATES.join("|");
    Regex::new(&format!(
        r"(?:^|[^\w:])(?:{crates})::|\bcrate::(?:ui|infra|app)\b|\bcrate::\{{[^}}]*\b(?:ui|infra|app)\b"
    ))
    .expect("valid regex")
}

/// The code part of a line: comments and doc comments may name other layers.
fn code_part(line: &str) -> &str {
    if line.trim_start().starts_with("//") {
        return "";
    }
    line.split(" //").next().unwrap_or(line)
}

#[test]
fn domain_does_not_depend_on_other_layers_or_toolkits() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src/domain"), &mut files);
    files.sort();
    assert!(!files.is_empty(), "found no files under src/domain");

    let re = forbidden();
    let mut found = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("read source file");
        for (idx, line) in text.lines().enumerate() {
            if re.is_match(code_part(line)) {
                let rel = file.strip_prefix(root).unwrap_or(file);
                found.push(format!("{}:{}: {}", rel.display(), idx + 1, line.trim()));
            }
        }
    }

    assert!(
        found.is_empty(),
        "`domain` must not depend on ui, infra, app or a toolkit crate:\n{}",
        found.join("\n")
    );
}

#[test]
fn layering_pattern_flags_violations_and_ignores_comments() {
    let re = forbidden();
    let hit = |s: &str| re.is_match(code_part(s));

    assert!(hit("use gtk::gdk;"));
    assert!(hit("    pub texture: gtk::gdk::Texture,"));
    assert!(hit("let t = gio::File::for_path(p);"));
    assert!(hit("use glib::SourceId;"));
    assert!(hit("use relm4::Sender;"));
    assert!(hit("use adw::prelude::*;"));
    assert!(hit("use crate::ui::constants;"));
    assert!(hit("use crate::infra::network::NetworkError;"));
    assert!(hit("use crate::app::state::FluxApp;"));
    assert!(hit("use crate::{domain::model::Config, infra::db};"));

    assert!(!hit("/// Wraps a `gio::Cancellable` without naming it."));
    assert!(!hit("// crate::ui is off limits here"));
    assert!(!hit("use crate::domain::model::Config;"));
    assert!(!hit("use tokio::sync::oneshot;"));
    assert!(!hit("let shadow::x = 1; // adw::"));
    assert!(!hit("use crate::application::Thing;"));
}
