//! Renderer for `docs/config.md`.

use super::Config;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Purpose of every key that appears in the serialized default config.
const FIELD_NOTES: &[(&str, &str)] = &[
    ("default_list_mode", "Open folders in list mode instead of grid mode."),
    ("network_bookmarks", "Saved network locations; see [Array tables](#array-tables)."),
    ("sidebar", "Custom sidebar entries; see [Array tables](#array-tables)."),
    ("ui.actions_panel_width", "Width in pixels of the right-side contextual-actions panel."),
    ("ui.archive_thumbnail_cache", "Store thumbnails of files inside zip archives in a hidden `.flux-cache/` folder in the zip itself."),
    ("ui.archives_as_folders", "Draw browsable archives (zip, tar, 7z and so on) as folders in the file grid, with a small archive badge. A selected card shows the archive's own icon. Archives remain files for drag and drop, delete and menu actions. Off by default."),
    ("ui.ascending", "Sort listings in ascending order."),
    ("ui.auto_generate_mime_icons", "Generate and cache an SVG document icon when the icon theme has none."),
    ("ui.auto_list_threshold", "Folders with more items than this open in list mode. `0` disables the auto-switch."),
    ("ui.auto_mime_accent_color", "Accent color of auto-generated document icons."),
    ("ui.auto_mime_body_color", "Hex color of the page body of auto-generated document icons."),
    ("ui.auto_mime_font_color", "Hex color of the extension label inside auto-generated document icons."),
    ("ui.auto_mime_font_size", "Base font size of auto-generated document icons."),
    ("ui.auto_show_diff", "Open the diff panel when a modified git file is selected."),
    ("ui.autoplay_video_previews", "Play a muted loop preview when a single video card is selected."),
    ("ui.bg_alpha_sidebar_left", "Tint opacity (0.0 to 1.0) over the left sidebar background image."),
    ("ui.bg_alpha_sidebar_right", "Tint opacity (0.0 to 1.0) over the right panel background image."),
    ("ui.bg_alpha_window", "Tint opacity (0.0 to 1.0) over the main window background image."),
    ("ui.breadcrumb_count", "Maximum number of path segments shown as breadcrumbs in the header bar; only the last ones are kept."),
    ("ui.breadcrumb_max_chars", "Characters shown in a breadcrumb label before truncation."),
    ("ui.content_search_max_file_mb", "Skip files larger than this many MiB during content search. `0` means no cap."),
    ("ui.current_folders_first", "Per-folder override of `folders_first`, keyed by absolute path."),
    ("ui.default_icon_size", "Default pixel size of file and folder icons."),
    ("ui.default_sort", "Fallback sort field: `Name`, `Date`, `Size` or `Type`."),
    ("ui.device_renames", "Display names and icons for devices and partitions; see [Array tables](#array-tables)."),
    ("ui.diff_editor", "Editor command used when opening a diff hunk location."),
    ("ui.diff_panel_width", "Width in pixels of the right-side git diff panel."),
    ("ui.disable_drag_and_drop", "Disable drag and drop of files and folders."),
    ("ui.enable_file_indexing", "Enable the FTS5 filename indexer."),
    ("ui.expand_labels", "Wrap filenames over multiple lines instead of truncating them."),
    ("ui.ffmpeg_auto_rotate", "Let FFmpeg rotate portrait video thumbnails."),
    ("ui.ffmpeg_seek_seconds", "Seek position in seconds when grabbing a video thumbnail frame."),
    ("ui.ffmpeg_threads", "Threads per FFmpeg process when extracting video frames."),
    ("ui.file_icons", "Per-file custom icon image, keyed by absolute path. Managed by `--set-icon`."),
    ("ui.fish_theme", "Generate a fish `conf.d` snippet so the embedded shell's colors and prompt follow the GTK theme. Off by default."),
    ("ui.folder_cache_capacity", "Number of recently visited directory snapshots kept in memory."),
    ("ui.folder_grid_thumbnails", "Show image thumbnails in the folder grid of the preview panel."),
    ("ui.folder_icon_size", "Per-folder icon size override, keyed by absolute path."),
    ("ui.folder_icons", "Per-folder custom icon image, keyed by absolute path. Managed by `--set-icon`."),
    ("ui.folder_sort", "Per-folder sort field override, keyed by absolute path."),
    ("ui.folders_first", "Group directories above files."),
    ("ui.grid_spacing", "Pixel spacing between items in the file grid."),
    ("ui.header_show_filter", "Show the filter button in the header bar. `Ctrl+Shift+F` toggles the filter bar while it is hidden."),
    ("ui.header_show_preview", "Show the preview-panel toggle in the header bar."),
    ("ui.header_show_sort", "Show the sort indicator in the header bar."),
    ("ui.header_show_view", "Show the list/grid toggle in the header bar. `Ctrl+Shift+,` toggles the mode while it is hidden."),
    ("ui.header_visible", "Show the header bar. Persisted across sessions."),
    ("ui.hidden_extensions", "Extensions to hide from grid labels, for example `[\"desktop\", \"AppImage\"]`."),
    ("ui.lazy_thumbnails", "Generate thumbnails only for items scrolled into view."),
    ("ui.list_icon_size", "Pixel size of icons in list mode."),
    ("ui.loader_batch_size", "Items dispatched per chunk into the grid while a folder loads."),
    ("ui.max_content_search_results", "Maximum results returned by content search."),
    ("ui.max_history", "Back and forward history entries kept."),
    ("ui.max_search_results", "Maximum matches during recursive filename and extension search."),
    ("ui.max_width_chars", "Characters shown in a file label before truncation."),
    ("ui.panel_menu_requires_ctrl", "Open left-sidebar, preview-panel and quick-list context menus only on Ctrl+right-click; a plain right-click does nothing."),
    ("ui.preview_panel_width", "Width in pixels of the right-side preview panel."),
    ("ui.preview_syntax_highlighting", "Syntax-highlight source files in the preview panel."),
    ("ui.recents_row", "Zero-based position of the Recents row among the `[[sidebar]]` entries. No effect when `show_recents` is `false`."),
    ("ui.scale_font_with_icons", "Scale label font size together with the grid icons."),
    ("ui.search_panel_width", "Width in pixels of the right-side search panel."),
    ("ui.show_csd", "Draw client-side window decorations."),
    ("ui.show_empty_dir_emblem", "Show an emblem on folders that contain no visible items."),
    ("ui.show_hidden_by_default", "Show hidden files at startup."),
    ("ui.show_recents", "Show the Recents location in the sidebar."),
    ("ui.show_symlink_emblem", "Show a badge and target hint on symbolic links."),
    ("ui.show_thumbnails", "Master switch for thumbnail generation; overrides `thumbnail_types`."),
    ("ui.show_xdg_dirs", "Show the XDG user directories, such as Documents and Downloads, in the sidebar."),
    ("ui.sidebar_menu_requires_ctrl", "Open sidebar context menus only on Ctrl+right-click; a plain right-click does nothing."),
    ("ui.sidebar_visible", "Show the sidebar. Persisted across sessions."),
    ("ui.sidebar_width", "Width in pixels of the left sidebar."),
    ("ui.single_click", "Activate items with a single click."),
    ("ui.start_maximized", "Open the window maximized."),
    ("ui.startup_window_height", "Initial window height in pixels."),
    ("ui.startup_window_width", "Initial window width in pixels."),
    ("ui.tag_panel_width", "Width in pixels of the right-side tag panel."),
    ("ui.terminal.bg_color", "Terminal background color."),
    ("ui.terminal.fg_color", "Terminal foreground color."),
    ("ui.terminal.font", "Terminal font as a Pango font description."),
    ("ui.terminal.height", "Terminal pane height."),
    ("ui.terminal.position", "Where F4 opens the terminal: `bottom` pane, right `sidebar` panel, or `fullscreen` window."),
    ("ui.thumbnail_size", "Target size in pixels of generated thumbnails."),
    ("ui.thumbnail_threads", "Maximum concurrent thumbnail workers."),
    ("ui.thumbnail_types.audio", "Thumbnails and embedded artwork for audio files."),
    ("ui.thumbnail_types.executables", "Thumbnails for Windows `.exe` files."),
    ("ui.thumbnail_types.fonts", "Thumbnails for font files."),
    ("ui.thumbnail_types.images", "Thumbnails for image files."),
    ("ui.thumbnail_types.pdfs", "Thumbnails for PDF documents."),
    ("ui.thumbnail_types.videos", "Thumbnails for video files."),
    ("ui.ui_scale", "Global UI scale multiplier."),
    ("ui.wide_preview", "Open the preview panel in wide mode, with the sidebar hidden."),
    ("ui.window_controls_left", "Put window controls on the left instead of the right."),
];

/// Optional keys that are absent from the serialized default config because they are unset.
const OPTIONAL_KEYS: &[(&str, &str)] = &[
    ("shortcuts.back", "Go back in navigation history."),
    ("shortcuts.change_icon", "Open the folder icon picker."),
    ("shortcuts.close_tab", "Close the current tab."),
    (
        "shortcuts.copy_path",
        "Copy the absolute paths of the selected items.",
    ),
    (
        "shortcuts.create_hardlink",
        "Create hardlinks from clipboard paths. Directories are not supported.",
    ),
    (
        "shortcuts.create_symlink",
        "Create symlinks from clipboard paths, auto-renaming conflicts.",
    ),
    ("shortcuts.cycle_sort", "Rotate through the sort fields."),
    ("shortcuts.delete", "Move the selected items to the trash."),
    ("shortcuts.forward", "Go forward in navigation history."),
    ("shortcuts.home", "Go to the home directory."),
    ("shortcuts.menu_editor", "Open the menu editor."),
    ("shortcuts.new_tab", "Open a new tab."),
    ("shortcuts.next_tab", "Switch to the next tab."),
    (
        "shortcuts.open",
        "Open the selected file or enter the directory.",
    ),
    ("shortcuts.open_properties", "Open the properties window."),
    ("shortcuts.prev_tab", "Switch to the previous tab."),
    ("shortcuts.quit", "Quit the application."),
    ("shortcuts.refresh", "Reload the current directory."),
    ("shortcuts.rename", "Start an inline rename."),
    (
        "shortcuts.reset_icon",
        "Reset a folder icon to the default.",
    ),
    ("shortcuts.root", "Go to the root directory."),
    ("shortcuts.search", "Focus the search interface."),
    ("shortcuts.settings", "Open the settings window."),
    (
        "shortcuts.toggle_folders_first",
        "Toggle folder grouping for the active folder.",
    ),
    ("shortcuts.toggle_header", "Show or hide the header bar."),
    ("shortcuts.toggle_hidden", "Show or hide hidden files."),
    (
        "shortcuts.toggle_sort_order",
        "Toggle between ascending and descending order.",
    ),
    (
        "ui.terminal.shell",
        "Shell started in the terminal pane; the user's login shell when unset.",
    ),
    (
        "ui.theme",
        "Name of the theme to apply, such as `default`, `os-dark` or `os-light`.",
    ),
];

/// A config key that was renamed or removed. Append new rows, never edit or delete old ones.
struct Migration {
    old: &'static str,
    new: &'static str,
    since: &'static str,
    note: &'static str,
}

/// Renamed and removed keys, oldest first.
const MIGRATIONS: &[Migration] = &[Migration {
    old: "ui.terminal_in_sidebar",
    new: "ui.terminal.position",
    since: "0.6.0",
    note: "Replaced by the three-way `position` enum. `true` loads as `\"sidebar\"`, `false` or missing as `\"bottom\"`.",
}];

/// Collects `(dotted key, default value text)` for every leaf of `value`.
fn collect_leaves(prefix: &str, value: &toml::Value, out: &mut Vec<(String, String)>) {
    match value {
        toml::Value::Table(table) if !table.is_empty() => {
            for (key, child) in table {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                collect_leaves(&path, child, out);
            }
        }
        _ => out.push((prefix.to_string(), value.to_string())),
    }
}

/// Returns the config as parsed from a file that has a `[ui]` table and nothing else.
fn defaults_config() -> Config {
    toml::from_str("[ui]\n").expect("an empty [ui] table always parses")
}

/// Returns every documented key with its default, grouped by TOML table.
fn sections() -> BTreeMap<String, Vec<(String, String)>> {
    let tree = toml::Value::try_from(defaults_config()).expect("Config always serializes");
    let mut leaves = Vec::new();
    collect_leaves("", &tree, &mut leaves);

    let has_optional_children = |key: &str| {
        OPTIONAL_KEYS.iter().any(|(k, _)| {
            k.strip_prefix(key)
                .is_some_and(|rest| rest.starts_with('.'))
        })
    };
    leaves.retain(|(key, _)| !has_optional_children(key));
    leaves.extend(
        OPTIONAL_KEYS
            .iter()
            .map(|(key, _)| (key.to_string(), "unset".to_string())),
    );
    leaves.sort();

    let mut grouped: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (key, default) in leaves {
        let (section, name) = key.rsplit_once('.').unwrap_or(("", key.as_str()));
        grouped
            .entry(section.to_string())
            .or_default()
            .push((name.to_string(), default));
    }
    grouped
}

/// Looks up the note for a dotted key.
fn note_for(key: &str) -> &'static str {
    FIELD_NOTES
        .iter()
        .chain(OPTIONAL_KEYS)
        .find(|(k, _)| *k == key)
        .map_or("", |(_, note)| *note)
}

/// Escapes characters that would break a Markdown table cell.
fn cell(s: &str) -> String {
    s.replace('|', "\\|")
}

/// Returns the complete `docs/config.md`, prose and generated tables.
#[allow(dead_code)] // used by `src/bin/gen-docs.rs`, not linked by the `flux-fm` binary alone
pub fn render_config_markdown() -> String {
    let mut out = String::new();

    out.push_str("# Configuration\n\n");
    out.push_str(
        "Flux reads `~/.config/flux/config.toml`. The file is created on first \
         run and reloaded while Flux is running. If it fails to parse, Flux \
         prints the error to stderr and runs with built-in fallback values \
         until the file is fixed.\n\n\
         Every key is optional. **Default** below is the value used when a key \
         is missing from a file that has a `[ui]` table. The file written on \
         first run sets some keys differently, so a fresh install can differ \
         from this table.\n\n",
    );

    out.push_str("## Reference\n\n");
    out.push_str("<!-- BEGIN GENERATED: keys -->\n");
    for (section, rows) in sections() {
        if section.is_empty() {
            out.push_str("### Top level\n\n");
        } else {
            let _ = writeln!(out, "### `[{section}]`\n");
        }
        out.push_str("| Key | Default | Description |\n");
        out.push_str("|---|---|---|\n");
        for (name, default) in rows {
            let key = if section.is_empty() {
                name.clone()
            } else {
                format!("{section}.{name}")
            };
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} |",
                name,
                cell(&default),
                cell(note_for(&key))
            );
        }
        out.push('\n');
    }
    out.push_str("<!-- END GENERATED: keys -->\n\n");

    out.push_str("## Array tables\n\n");
    out.push_str(
        "`[[sidebar]]` adds a sidebar entry. `path` accepts a leading `~`. \
         Set `kind = \"label\"` to make the entry a section heading instead of \
         a location.\n\n\
         ```toml\n\
         [[sidebar]]\n\
         name = \"Projects\"\n\
         icon = \"folder-symbolic\"\n\
         path = \"~/Projects\"\n\
         ```\n\n\
         `[[network_bookmarks]]` saves a network location. `icon` is optional.\n\n\
         ```toml\n\
         [[network_bookmarks]]\n\
         name = \"NAS\"\n\
         uri = \"smb://nas.local/share\"\n\
         ```\n\n\
         `[ui.device_renames]` maps a device path to a display name and an \
         optional icon:\n\n\
         ```toml\n\
         [ui.device_renames]\n\
         \"/path/to/device/\" = { name = \"Storage\", icon = \"drive-harddisk-solid-symbolic\" }\n\
         ```\n\n",
    );

    out.push_str("## Default configuration\n\n");
    out.push_str(
        "The same defaults as one file, as Flux would serialize it. Keys that \
         are unset, such as `ui.theme` and the `[shortcuts]` entries, are \
         omitted.\n\n",
    );
    out.push_str("<!-- BEGIN GENERATED: default-toml -->\n");
    out.push_str("```toml\n");
    out.push_str(&toml::to_string_pretty(&defaults_config()).expect("Config always serializes"));
    out.push_str("```\n");
    out.push_str("<!-- END GENERATED: default-toml -->\n\n");

    out.push_str("## Renamed and removed keys\n\n");
    if MIGRATIONS.is_empty() {
        out.push_str("No keys have been renamed or removed yet.\n");
    } else {
        out.push_str("| Old key | Replaced by | Since | Note |\n");
        out.push_str("|---|---|---|---|\n");
        for m in MIGRATIONS {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                m.old,
                if m.new.is_empty() {
                    "removed".to_string()
                } else {
                    format!("`{}`", m.new)
                },
                m.since,
                m.note
            );
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf_keys() -> Vec<String> {
        sections()
            .into_iter()
            .flat_map(|(section, rows)| {
                rows.into_iter().map(move |(name, _)| {
                    if section.is_empty() {
                        name
                    } else {
                        format!("{section}.{name}")
                    }
                })
            })
            .collect()
    }

    #[test]
    fn doc_is_in_sync() {
        let generated = render_config_markdown();
        let committed = std::fs::read_to_string("docs/config.md")
            .expect("docs/config.md is missing; create it from render_config_markdown()");
        assert_eq!(
            generated, committed,
            "\ndocs/config.md is stale.\n\
             Run `cargo run --bin gen-docs` to regenerate it."
        );
    }

    #[test]
    fn every_key_has_a_note() {
        for key in leaf_keys() {
            assert!(
                !note_for(&key).is_empty(),
                "{key} is in the default config but has no entry in FIELD_NOTES"
            );
        }
    }

    #[test]
    fn every_note_names_a_real_key() {
        let keys = leaf_keys();
        for (key, _) in FIELD_NOTES.iter().chain(OPTIONAL_KEYS) {
            assert!(
                keys.iter().any(|k| k == key),
                "{key} has a note but is not a key of the default config"
            );
        }
    }

    #[test]
    fn optional_keys_are_unset_in_the_default_config() {
        let tree = toml::Value::try_from(defaults_config()).unwrap();
        let mut leaves = Vec::new();
        collect_leaves("", &tree, &mut leaves);
        for (key, _) in OPTIONAL_KEYS {
            assert!(
                !leaves.iter().any(|(k, _)| k == key),
                "{key} is set in the default config; move it to FIELD_NOTES"
            );
        }
    }

    #[test]
    fn optional_keys_are_real_fields() {
        for (key, _) in OPTIONAL_KEYS {
            let mut path: Vec<&str> = key.split('.').collect();
            let leaf = path.pop().unwrap();
            let mut inner = toml::Table::new();
            inner.insert(leaf.to_string(), toml::Value::String("probe".into()));
            let mut value = toml::Value::Table(inner);
            for part in path.into_iter().rev() {
                let mut outer = toml::Table::new();
                outer.insert(part.to_string(), value);
                value = toml::Value::Table(outer);
            }

            let config: Config = value.try_into().unwrap();
            let tree = toml::Value::try_from(config).unwrap();
            let mut leaves = Vec::new();
            collect_leaves("", &tree, &mut leaves);
            assert!(
                leaves.iter().any(|(k, _)| k == key),
                "{key} is in OPTIONAL_KEYS but is not a field of Config"
            );
        }
    }
}
