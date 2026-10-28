//! Background warm-up of the files GTK reads when it first resolves an icon.

use gtk::glib;
use std::collections::VecDeque;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// Most themes visited through `Inherits=`, which also bounds inheritance cycles.
const MAX_THEMES: usize = 16;
/// Directory depth listed for a theme without a cache (`48x48/apps` is depth 2).
const LIST_DEPTH: usize = 2;
/// Most directory entries listed across all cache-less themes.
const MAX_ENTRIES: usize = 50_000;
/// Theme warmed when the settings do not name one.
const FALLBACK_THEME: &str = "Adwaita";

/// Starts warming the active icon theme's files on a worker thread.
pub fn preload_icon_theme() {
    crate::hit!("preload_icon_theme");
    let theme = gtk::Settings::default()
        .and_then(|s| s.gtk_icon_theme_name())
        .map(|n| n.to_string());
    let _ = std::thread::Builder::new()
        .name("flux-icon-preload".into())
        .spawn(move || warm(theme.as_deref().unwrap_or(FALLBACK_THEME)));
}

/// Reads the files GTK would read for `theme`, its parents and `hicolor`.
fn warm(theme: &str) {
    let mut roots = vec![
        glib::user_data_dir().join("icons"),
        glib::home_dir().join(".icons"),
    ];
    roots.extend(
        glib::system_data_dirs()
            .into_iter()
            .map(|d| d.join("icons")),
    );

    let mut budget = MAX_ENTRIES;
    for name in theme_chain(&roots, theme) {
        for root in &roots {
            warm_theme_dir(&root.join(&name), &mut budget);
        }
    }
}

/// Returns the `Inherits=` names from the text of an `index.theme`.
fn inherits(index_theme: &str) -> Vec<String> {
    index_theme
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(key, list)| (key.trim() == "Inherits").then_some(list))
        .map(|list| {
            list.split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Returns `first`, the themes it inherits from, and `hicolor`, without duplicates.
fn theme_chain(roots: &[PathBuf], first: &str) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut queue = VecDeque::from([first.to_string()]);
    while let Some(name) = queue.pop_front() {
        if chain.len() >= MAX_THEMES || chain.contains(&name) {
            continue;
        }
        let parents = roots
            .iter()
            .find_map(|r| std::fs::read_to_string(r.join(&name).join("index.theme")).ok())
            .map(|text| inherits(&text))
            .unwrap_or_default();
        queue.extend(parents);
        chain.push(name);
    }
    if !chain.iter().any(|n| n == "hicolor") {
        chain.push("hicolor".to_string());
    }
    chain
}

/// Pulls one theme directory into the page cache: its cache file when it has
/// one, otherwise the directory tree GTK would scan instead.
fn warm_theme_dir(dir: &Path, budget: &mut usize) {
    match File::open(dir.join("icon-theme.cache")) {
        Ok(mut cache) => {
            let _ = io::copy(&mut cache, &mut io::sink());
        }
        Err(_) => list_dir(dir, LIST_DEPTH, budget),
    }
}

/// Lists `dir` and its subdirectories down to `depth`, spending `budget` entries.
fn list_dir(dir: &Path, depth: usize, budget: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if depth > 0 && entry.file_type().is_ok_and(|t| t.is_dir()) {
            list_dir(&entry.path(), depth - 1, budget);
        }
    }
}
