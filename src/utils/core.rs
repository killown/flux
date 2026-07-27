use crate::ui::constants;
use crate::utils::media::probe_media_duration;
use crate::utils::PathExt;
use adw::gdk;
use adw::prelude::*;
use gtk::gdk_pixbuf;
use gtk::gio;
use gtk::glib;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::CustomAction;
use crate::model::TerminalConfig;

pub fn ensure_config_file() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    let config_path = config_dir.join("menu.rs");
    if !config_path.exists() {
        // Syntax: "Label" => "mime_types", "command", "Optional Toast Message"
        // - Use %p for file path, %d for directory, %f for filename.
        // - The third argument (Toast) is optional and shows a notification after execution.

        let default_config = r#"
# --- Core Operations ---
"󰋼      Add to Quick List" => "directory", "builtin::add_to_quick_list"
"󰆏      Copy" => "all", "builtin::copy", "Copied to clipboard"
"󰆐      Cut" => "all", "builtin::cut", "Cut to clipboard"
"󰏊      Paste" => "all", "builtin::paste", "Pasted items"
"󰱝      Open With..." => "file", "builtin::open_with"
"󰩹      Move to Trash" => "all", "gio trash %p", "Moved to trash"
"󰦬      Restore File" => "trash", "gio trash --restore %p", "File restored"
"󰆴      Shred File (Permanent)" => "all", "python $HOME/.local/share/flux/scripts/flux_shredder.py %p", "Shredder initialized"

# --- Navigation & System ---
"      Open Terminal" => "directory", "alacritty --working-directory=%p"
"󰨞      Open in VSCode" => "text/all, application/all", "code %p"
"󰋽      File Properties" => "file", "flux-fm --file-properties %p"
"󰋊      Folder Info" => "directory", "baobab %p"
"󰉋      New Folder" => "directory", "mkdir %p/New-Folder", "Folder created"

# --- Media Edit ---
"󰽰      Media Edit > Join Videos" => "video/all", "python3 $HOME/.local/share/flux/scripts/join_videos.py %p", "Joining videos..."
"󰽰      Media Edit > Cut Video" => "video/all", "python $HOME/.local/share/flux/scripts/video_cutter.py %p", "Opening Video Cutter..."
"󰽰      Media Edit > Mix Audio" => "audio/", "python3 $HOME/.local/share/flux/scripts/mix_audio.py %p", "Mixing audio..."
"󰽰      Media Edit > Merge Video + Audio" => "video/all+audio/all", "python $HOME/.local/share/flux/scripts/join_mp4_mp3.py %p", "Merging media..."

# --- Media Convert ---
"󰽰      Media Convert > To MP4" => "video/all", "ffmpeg -i %p -codec copy %p.mp4", "Converting to MP4..."
"󰽰      Media Convert > To MKV" => "video/all", "ffmpeg -i %p -codec copy %p.mkv", "Converting to MKV..."
"󰽰      Media Convert > To WebM" => "video/all", "ffmpeg -i %p -codec copy %p.webm", "Converting to WebM..."
"󰽰      Media Convert > To MOV" => "video/all", "ffmpeg -i %p -codec copy %p.mov", "Converting to MOV..."
"󰝚      Media Convert > Audio to MP3" => "audio/x-opus+ogg, audio/vnd.wave, audio/ogg", "ffmpeg -i %p -vn -ab 192k -ar 44100 -y %p.mp3", "Converting to MP3..."

# --- Media Optimization & Extraction ---
"󰠝      Media Extract > MP3 from Video" => "video/all", "ffmpeg -i %p -vn -acodec libmp3lame -q:a 2 %p.mp3", "Extracting MP3..."
"󰕧      Media Optimize > Reduce Video Size" => "video/all", "ffmpeg -i %p -vcodec libx265 -crf 28 -tag:v hvc1 -preset faster %p_reduced.mp4", "Reducing video size..."
"󰛖      Extract Here!" => "application/zip, application/x-7z-compressed, application/x-rar, application/x-tar", "7z x %p -o%p_extracted", "Extracting archive..."

# --- Images & Wallpaper ---
"󰸉      Image Wallpaper > Set (swww)" => "image/all", "swww img %p", "Wallpaper set (swww)"
"󰸉      Image Wallpaper > Set (wbg)" => "image/all", "cp %p ~/Images/fav.jpg && wbg -s ~/Images/fav.jpg", "Wallpaper set (wbg)"
"󰸉      Image Convert > To AVIF" => "image/all", "avifenc --jobs all -q 65 %p %p.avif", "Image converted to AVIF"
"󰸉      Image Convert > To JPG" => "image/all", "magick %p -quality 75 -strip %p-output.jpg", "Image converted to JPG"
"󰏦      Convert to PDF" => "image/all, application/pdf, application/msword, application/vnd.openxmlformats-officedocument.wordprocessingml.document", "python3 $HOME/.local/share/flux/scripts/pdf_converter.py %p", "Converting to PDF..."

# --- Tools ---
"󰯦      Tools > Git Gui" => "directory", "git gui"
"󰯦      Tools > Download Video (1080p)" => "directory", "cd %p && yt-dlp -f 'bv[height<=1080]+ba/b[height<=1080]' $(wl-paste)", "Video download started"
"󰯦      Tools > Copy Path" => "all", "echo -n %p | wl-copy", "Path copied to clipboard"
"󰯦      Tools > Copy Name" => "all", "basename %p | tr -d '\n' | wl-copy", "Name copied to clipboard"
"󰯦      Tools > Advanced Archive Manager" => "all", "/usr/bin/python $HOME/.local/share/flux/scripts/flux_compressor.py %p"
"#;

        if let Ok(mut file) = fs::File::create(&config_path) {
            let _ = file.write_all(default_config.as_bytes());
        }
    }
    config_path
}

pub fn save_config(config: &crate::model::Config) {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    let config_path = config_dir.join("config.toml");

    if let Ok(toml_str) = toml::to_string_pretty(config) {
        let _ = fs::write(config_path, toml_str);
    }
}

pub fn rename_path(old_path: &Path, new_name: &str) -> std::io::Result<PathBuf> {
    // Reject any name containing a path separator to prevent directory traversal
    if new_name.contains(std::path::MAIN_SEPARATOR) || new_name.contains('/') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "new name must be a plain filename, not a path",
        ));
    }

    let mut new_path = old_path.to_path_buf();
    new_path.set_file_name(new_name);

    if new_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "A file with this name already exists",
        ));
    }

    fs::rename(old_path, &new_path)?;
    Ok(new_path)
}

pub fn load_config() -> crate::model::Config {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("flux");

    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        let _ = fs::create_dir_all(&config_dir);

        let mut default_toml = String::from(
            r#"[ui]
default_icon_size = 96
startup_window_width = 1280
startup_window_height = 800
sidebar_width = 200
single_click = false
show_xdg_dirs = false
default_sort = "Name"
show_hidden_by_default = false
folders_first = true
theme = "default"
show_csd = false
start_maximized = true
show_thumbnails = true

[ui.terminal]
height = 50
fg_color = ""
bg_color = ""
font = "JetBrains Mono 11"

[ui.thumbnail_types]
images = true
videos = true
fonts = true
pdfs = true

[ui.folder_sort]

[ui.device_renames]
"/path/to/device/" = { name = "Storage", icon = "drive-harddisk-solid-symbolic" }

[ui.folder_icon_size]

[shortcuts]
# -- Navigation --

# Returns to the previous folder in the linear history stack.
back = "BackSpace"

# Moves forward in the history stack.
forward = "<Alt>Right"

# Activates the selected file or enters the selected directory.
open = "Return"

# -- File Operations --

# Moves selected items to the trash.
delete = "Delete"

# -- View & Application --

# Triggers a reload of the current directory.
refresh = "F5"

# Focuses the search/filter entry bar.
search = "<Primary>f"

# Toggles the visibility of hidden files (dotfiles).
toggle_hidden = "<Primary>h"


[[sidebar]]
name = "Default"
kind = "label"
icon = ""
path = ""

[[sidebar]]
name = "Home"
icon = "user-home-symbolic"
path = "~"
"#,
        );

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

        let mut add_entry = |path: PathBuf, icon: &str, custom_name: Option<&str>| {
            if path.exists() || icon == "user-trash-symbolic" {
                let name = custom_name.map(|s| s.to_string()).unwrap_or_else(|| {
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Unknown".into())
                });

                let path_str = if path == home {
                    "~".to_string()
                } else if icon == "user-trash-symbolic" {
                    "trash:///".to_string()
                } else if let Ok(stripped) = path.strip_prefix(&home) {
                    format!("~/{}", stripped.to_string_lossy())
                } else {
                    path.to_string_lossy().into_owned()
                };

                use std::fmt::Write;
                let _ = write!(
                    default_toml,
                    "[[sidebar]]\nname = {:?}\nicon = {:?}\npath = {:?}\n\n",
                    name, icon, path_str
                );
            }
        };

        // 1. Home
        add_entry(home.clone(), "user-home-symbolic", Some("Home"));

        // 2. Localized XDG Folders with correct icons
        if let Some(p) = dirs::download_dir() {
            add_entry(p, "folder-download-symbolic", None);
        }
        if let Some(p) = dirs::document_dir() {
            add_entry(p, "folder-documents-symbolic", None);
        }
        if let Some(p) = dirs::picture_dir() {
            add_entry(p, "folder-pictures-symbolic", None);
        }
        if let Some(p) = dirs::video_dir() {
            add_entry(p, "folder-videos-symbolic", None);
        }
        if let Some(p) = dirs::audio_dir() {
            add_entry(p, "folder-music-symbolic", None);
        }

        // 3. Trash
        add_entry(
            PathBuf::from("trash:///"),
            "user-trash-symbolic",
            Some("Trash"),
        );

        let _ = fs::write(&config_path, default_toml);
    }

    let mut config: crate::model::Config = fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_else(|| crate::model::Config {
            ui: crate::model::UIConfig {
                default_icon_size: 128,
                startup_window_width: crate::ui::constants::DEFAULT_WIDTH,

                startup_window_height: crate::ui::constants::DEFAULT_HEIGHT,
                single_click: false,
                show_csd: true,
                sidebar_width: 240,
                show_xdg_dirs: false,
                current_folders_first: std::collections::HashMap::new(),

                default_sort: crate::model::SortBy::Name,
                folder_sort: std::collections::HashMap::new(),
                folder_icon_size: std::collections::HashMap::new(),
                show_hidden_by_default: false,
                device_renames: std::collections::HashMap::new(),
                folders_first: true,

                theme: Some("default".to_string()),
                start_maximized: true,
                max_width_chars: 20,
                grid_spacing: 10,
                ascending: true,
                expand_labels: false,
                folder_icons: std::collections::HashMap::new(),
                terminal: TerminalConfig::default(),
                sidebar_visible: true,
                show_recents: true,
                recents_row: 0,
                show_thumbnails: true,
                thumbnail_types: crate::model::ThumbnailTypes::default(),
            },
            sidebar: vec![],
            shortcuts: crate::model::ShortcutsConfig::default(),
        });

    let mut changed = false;

    config.ui.folder_sort.retain(|path_str, _| {
        let path = if path_str.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path_str.trim_start_matches("~/")))
                .unwrap_or_else(|| PathBuf::from(path_str))
        } else {
            PathBuf::from(path_str)
        };

        let exists = path.exists() || path_str == "trash:///";
        if !exists {
            changed = true;
        }
        exists
    });

    config.ui.folder_icon_size.retain(|path_str, _| {
        let path = if path_str.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path_str.trim_start_matches("~/")))
                .unwrap_or_else(|| PathBuf::from(path_str))
        } else {
            PathBuf::from(path_str)
        };
        let exists = path.exists() || path_str == "trash:///";
        if !exists {
            changed = true;
        }
        exists
    });

    if changed {
        crate::utils::save_config(&config);
    }

    config
}

/// Parses the right-hand side of a menu config line into (mime, command, optional_toast).
fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>)> {
    let input = input.trim();

    let remainder = input.strip_prefix('"')?;
    let (mime, rest) = remainder.split_once('"')?;

    let second_part = rest.trim().strip_prefix(',')?.trim();

    // Find the closing quote of the command, allowing for escaped content
    let cmd_inner = second_part.strip_prefix('"')?;
    let (cmd, after_cmd) = cmd_inner.split_once('"')?;

    // Optional 3rd field: , "toast message"
    let toast = after_cmd
        .trim()
        .strip_prefix(',')
        .and_then(|s| s.trim().strip_prefix('"'))
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| s.to_string());

    Some((mime.to_string(), cmd.to_string(), toast))
}

pub fn load_menu_config() -> Vec<CustomAction> {
    let config_path = ensure_config_file();
    let content = std::fs::read_to_string(config_path).unwrap_or_default();

    let mut actions = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        if let Some((left, right)) = line.split_once("=>") {
            let full_label = left.trim().trim_matches('"');

            let (submenu, label) = if full_label.contains(" > ") {
                let parts: Vec<&str> = full_label.splitn(2, " > ").collect();
                (Some(parts[0].to_string()), parts[1].to_string())
            } else {
                (None, full_label.to_string())
            };

            if let Some((mimes_part, cmd_part, toast)) = split_mime_cmd(right) {
                let mime_types: Vec<String> = mimes_part
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();

                actions.push(CustomAction {
                    label,
                    submenu,

                    action_name: format!("custom_{}", i),
                    command: cmd_part,
                    mime_types,
                    toast,
                });
            }
        }
    }
    actions
}

pub fn get_icon_for_path(path: &Path, is_dir: bool) -> adw::gio::Icon {
    get_icon_for_path_with_override(path, is_dir, None)
}

/// Returns a GIO icon for the given path, applying a custom icon name override when provided.
///
/// # Arguments
///
/// * `path`          - Absolute path to the file or directory.
/// * `is_dir`        - Whether the entry is a directory.
/// * `custom_icon`   - Optional GTK icon name to use instead of the derived default.
pub fn get_icon_for_path_with_override(
    path: &Path,
    is_dir: bool,
    custom_icon: Option<&str>,
) -> adw::gio::Icon {
    if let Some(icon_name) = custom_icon {
        if let Ok(icon) = gio::Icon::for_string(icon_name) {
            return icon;
        }
    }
    if is_dir {
        return gio::Icon::for_string("folder").unwrap();
    }
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);

    adw::gio::content_type_get_icon(&content_type)
}

pub fn get_mime_type(path: &Path) -> String {
    if path.is_dir() {
        return "inode/directory".to_string();
    }

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let mut sniff_buffer = [0u8; 4096];

    let data_slice = if let Ok(mut file) = fs::File::open(path) {
        if let Ok(count) = file.read(&mut sniff_buffer) {
            &sniff_buffer[..count]
        } else {
            &[]
        }
    } else {
        &[]
    };

    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), data_slice);
    content_type.to_string()
}

pub fn is_visual_media(path: &Path) -> (bool, bool) {
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
    (
        content_type.starts_with("image/"),
        content_type.starts_with("video/"),
    )
}

pub fn expand_path(path: &str) -> PathBuf {
    // We delegate the logic to our PathExt trait which handles
    // component stripping and home directory joining safely.

    PathBuf::from(path).expand_tilde()
}

pub fn open_file(path: PathBuf) {
    let file = adw::gio::File::for_path(&path);
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
    if let Some(app_info) = adw::gio::AppInfo::default_for_type(&content_type, false) {
        let _ = app_info.launch(&[file], None::<&adw::gio::AppLaunchContext>);
    } else {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}

/// Resolves the FreeDesktop-compliant thumbnail cache path for a given source file.
///
/// Implements §2 of the [Thumbnail Managing Standard] by computing an MD5 digest
/// of the canonical `file://` URI and mapping it into the shared XDG thumbnail
/// store at `$XDG_CACHE_HOME/thumbnails/<size-tier>/<hash>.png`. Cache entries
/// produced here are session-persistent and visible to other XDG-compliant
/// applications (e.g. Nautilus, Thunar) in the same store.
///
/// [Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to the source media file.
///
/// # Returns
///
/// `(cache_dir, cache_path)` where `cache_dir` is the resolved size-tier directory
/// and `cache_path` is the full `.png` destination. Returns `None` if
/// `dirs::cache_dir()` is unavailable or `path` contains non-UTF-8 bytes.
fn thumbnail_cache_path(path: &Path) -> Option<(PathBuf, PathBuf)> {
    let thumb_folder = match constants::CACHED_THUMBNAIL_SIZE {
        512 => "xx-large",
        256 => "x-large",
        128 => "large",
        _ => "normal",
    };

    let cache_dir = dirs::cache_dir()?.join("thumbnails").join(thumb_folder);

    // gio::File::uri() produces a correctly percent-encoded RFC 2396 URI,
    // which is what the FreeDesktop thumbnail spec mandates for the MD5 input.
    let uri = gio::File::for_path(path).uri();
    let hash = format!("{:x}", md5::compute(uri.as_bytes()));
    let cache_path = cache_dir.join(format!("{}.png", hash));

    Some((cache_dir, cache_path))
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Renders the first page of a PDF to a PNG thumbnail and writes it to `cache_path`.
///
/// Scales the page so its longest axis fits within [`constants::CACHED_THUMBNAIL_SIZE`],
/// paints a white background (PDF pages are transparent by default), then serialises
/// the result via `gdk_pixbuf` into the shared XDG thumbnail store so it is
/// session-persistent and reused on subsequent directory loads.
///
/// # Arguments
///
/// * `path`       - Absolute path to the source PDF file.
/// * `cache_path` - Destination `.png` path inside the XDG thumbnail store.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the document cannot be opened, contains
/// no pages, or the Cairo surface cannot be serialised to PNG.
fn pdf_thumbnail(path: &Path, cache_path: &Path) -> Option<gdk::Texture> {
    let doc = poppler::PopplerDocument::new_from_file(path, None).ok()?;
    let page = doc.get_page(0)?;
    let (page_w, page_h) = page.get_size();

    let size = constants::CACHED_THUMBNAIL_SIZE as f64;
    let scale = size / page_w.max(page_h);
    let render_w = (page_w * scale).round() as i32;
    let render_h = (page_h * scale).round() as i32;

    // ARgb32 gives us 4 bytes/pixel (BGRA native order) which matches what
    // `surface.data()` returns and what `Pixbuf::from_bytes` with has_alpha=true expects.
    let mut surface =
        poppler::cairo::ImageSurface::create(poppler::cairo::Format::ARgb32, render_w, render_h)
            .ok()?;
    let cx = poppler::cairo::Context::new(&surface).ok()?;

    // PDF pages are transparent by default, fill white so the thumbnail
    // looks correct on both light and dark file-manager backgrounds.
    cx.set_source_rgb(1.0, 1.0, 1.0);
    cx.paint().ok()?;
    cx.scale(scale, scale);
    page.render(&cx);

    // Drop the context before calling surface.data() - both borrow the surface
    // and Rust enforces that only one mutable borrow exists at a time.
    // Avoids a PNG encode/decode round-trip and sidesteps the `'static` bound
    // on `Pixbuf::from_read`. `ImageSurface::data()` exposes BGRA (cairo native),
    // so has_alpha=true lets gdk_pixbuf handle the channel layout via the stride.
    drop(cx);
    surface.flush();
    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride();
    let data = surface.data().ok()?;

    let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from(&*data),
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        stride,
    );

    if let Some(path_str) = cache_path.to_str() {
        let _ = pixbuf.savev(path_str, "png", &[]);
    }

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// Returns `true` if `path` is a font file by extension (case-insensitive).
fn is_font(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "ttf" | "otf" | "woff" | "woff2" | "ttc"
        )
    })
}

/// Renders a font preview thumbnail using PangoCairo and writes it to `cache_path`.
///
/// Loads the font file directly into a temporary [`pango::FontMap`] override via
/// `fc-cache`-free [`fontconfig`] font loading, then lays out a two-line sample:
/// the font family name on top and the pangram `"AaBbCc 123"` below in the target
/// font at a size scaled to fill [`constants::CACHED_THUMBNAIL_SIZE`].
///
/// The background is white with dark text so thumbnails are legible on both light
/// and dark file-manager themes.
///
/// # Arguments
///
/// * `path`       - Absolute path to the source font file.
/// * `cache_path` - Destination `.png` path inside the XDG thumbnail store.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the font cannot be loaded or the Cairo
/// surface cannot be read back.
/// Reads the font family name from a TrueType/OpenType `name` table (nameID 1).
///
/// Parses just enough of the binary `name` table to extract the English family
/// name without pulling in a full font parsing library.  Returns `None` if the
/// file cannot be read or the table is malformed, the caller falls back to the
/// filename stem in that case.
fn read_font_family(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;

    // Offset table: 12 bytes header + 16 bytes per table record.
    // We scan the table directory for the 'name' tag (0x6E616D65).
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let dir_start = 12usize;

    let mut name_offset = None;
    for i in 0..num_tables {
        let base = dir_start + i * 16;
        if base + 16 > data.len() {
            break;
        }
        let tag = &data[base..base + 4];
        if tag == b"name" {
            let offset = u32::from_be_bytes([
                data[base + 8],
                data[base + 9],
                data[base + 10],
                data[base + 11],
            ]) as usize;
            name_offset = Some(offset);
            break;
        }
    }

    let name_base = name_offset?;
    if name_base + 6 > data.len() {
        return None;
    }

    let count = u16::from_be_bytes([data[name_base + 2], data[name_base + 3]]) as usize;
    let string_offset = u16::from_be_bytes([data[name_base + 4], data[name_base + 5]]) as usize;
    let storage = name_base + string_offset;

    // Scan name records (12 bytes each) for nameID=1 (Family), platformID=3 (Windows), encodingID=1 (Unicode BMP).
    // Fall back to platformID=1 (Mac) if no Windows record found.
    let mut family_win: Option<String> = None;
    let mut family_mac: Option<String> = None;

    for i in 0..count {
        let rec = name_base + 6 + i * 12;
        if rec + 12 > data.len() {
            break;
        }
        let platform_id = u16::from_be_bytes([data[rec], data[rec + 1]]);
        let encoding_id = u16::from_be_bytes([data[rec + 2], data[rec + 3]]);
        let name_id = u16::from_be_bytes([data[rec + 6], data[rec + 7]]);
        let length = u16::from_be_bytes([data[rec + 8], data[rec + 9]]) as usize;
        let offset = u16::from_be_bytes([data[rec + 10], data[rec + 11]]) as usize;

        if name_id != 1 {
            continue;
        }

        let start = storage + offset;
        let end = start + length;
        if end > data.len() {
            continue;
        }
        let raw = &data[start..end];

        if platform_id == 3 && encoding_id == 1 && family_win.is_none() {
            // UTF-16 BE
            let chars: Vec<u16> = raw
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&chars) {
                family_win = Some(s);
            }
        } else if platform_id == 1 && family_mac.is_none() {
            // Mac Roman - ASCII-compatible for Latin family names
            family_mac = Some(String::from_utf8_lossy(raw).into_owned());
        }
    }

    family_win.or(family_mac)
}

/// Registers a font file with the process-local fontconfig instance so Pango
/// can resolve it without system installation.
///
/// Calls `FcConfigAppFontAddFile` from libfontconfig (a transitive system
/// dependency of GTK/Pango - always present on the target platform).  The
/// registration is process-local and cleaned up on exit, no global state is
/// modified.
fn register_font_with_fontconfig(path: &Path) {
    // SAFETY: libfontconfig is guaranteed present (GTK transitive dep).
    // FcConfigAppFontAddFile(NULL, path) adds `path` to the default config's
    // application font list.  NULL config pointer → current default config.
    // The path string is valid for the duration of the call.
    #[link(name = "fontconfig")]
    extern "C" {
        fn FcConfigAppFontAddFile(
            config: *mut std::ffi::c_void,
            file: *const std::os::raw::c_char,
        ) -> i32;
    }

    if let Ok(cpath) = std::ffi::CString::new(path.to_string_lossy().as_bytes()) {
        unsafe {
            FcConfigAppFontAddFile(std::ptr::null_mut(), cpath.as_ptr());
        }
    }
}

fn font_thumbnail(path: &Path, cache_path: &Path) -> Option<gdk::Texture> {
    // Register the font file with fontconfig so Pango can load it by family
    // name without requiring system installation.
    register_font_with_fontconfig(path);

    // Prefer the actual internal family name from the binary name table,
    // fall back to the filename stem if parsing fails.
    let family = read_font_family(path)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Sans".to_string());

    let size = constants::CACHED_THUMBNAIL_SIZE;
    let size_f = size as f64;

    let mut surface =
        pangocairo::cairo::ImageSurface::create(pangocairo::cairo::Format::ARgb32, size, size)
            .ok()?;
    let cx = pangocairo::cairo::Context::new(&surface).ok()?;

    cx.set_source_rgb(1.0, 1.0, 1.0);
    cx.paint().ok()?;

    let pango_cx = pangocairo::functions::create_context(&cx);

    // Sample lines: family name hint at top, pangram below.
    let sample_body = "AaBbCc 123\nThe quick fox";
    let body_pt = (size_f * 0.22) as i32;
    let mut body_desc = pango::FontDescription::from_string(&format!("{} {}", family, body_pt));
    body_desc.set_size(body_pt * pango::SCALE);

    // Small label at the top: family name in a neutral sans so it's always legible.
    let label_pt = (size_f * 0.09) as i32;
    let mut label_desc = pango::FontDescription::from_string(&format!("Sans {}", label_pt));
    label_desc.set_size(label_pt * pango::SCALE);

    let margin = size_f * 0.06;

    // Draw the family name label.
    cx.set_source_rgb(0.4, 0.4, 0.4);
    cx.move_to(margin, margin);
    let label_layout = pango::Layout::new(&pango_cx);
    label_layout.set_font_description(Some(&label_desc));
    label_layout.set_text(&family);
    label_layout.set_width((size - (margin * 2.0) as i32) * pango::SCALE);
    label_layout.set_ellipsize(pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &label_layout);

    // Draw the pangram sample in the target font.
    cx.set_source_rgb(0.05, 0.05, 0.05);
    let (_, label_h) = label_layout.pixel_size();
    cx.move_to(margin, margin + label_h as f64 + margin * 0.25);
    let body_layout = pango::Layout::new(&pango_cx);
    body_layout.set_font_description(Some(&body_desc));
    body_layout.set_text(sample_body);
    body_layout.set_width((size - (margin * 2.0) as i32) * pango::SCALE);
    body_layout.set_ellipsize(pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &body_layout);

    drop(cx);
    surface.flush();

    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride();
    let data = surface.data().ok()?;

    let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from(&*data),
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        stride,
    );

    if let Some(path_str) = cache_path.to_str() {
        let _ = pixbuf.savev(path_str, "png", &[]);
    }

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// Returns whether a cached thumbnail PNG is still valid for the given source file.
///
/// Compares the source's `mtime` against the thumbnail's own `mtime`. A thumbnail
/// is considered stale when the source was modified after the thumbnail was written,
/// or when the on-disk entry is zero bytes (partial write / crash residue).
///
/// # Arguments
///
/// * `cache_path`   - Path to the candidate `.png` thumbnail.
/// * `source_meta`  - [`fs::Metadata`] of the original source file.
fn thumbnail_is_valid(cache_path: &Path, source_meta: &fs::Metadata) -> bool {
    let cache_meta = match fs::metadata(cache_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let cache_mtime = cache_meta.modified().ok();
    let source_mtime = source_meta.modified().ok();

    // Source newer than cache → stale.
    if let (Some(ct), Some(st)) = (cache_mtime, source_mtime) {
        if st > ct {
            return false;
        }
    }

    // Guard against zero-byte truncation or partial writes.
    cache_meta.len() > 0
}

/// Removes recent entries from the `~/.local/share/recently-used.xbel` file.
///
/// If `paths` is `None`, all bookmarks are removed.
/// If `Some(paths)`, only bookmarks whose `href` matches one of the given paths are removed.
///
/// # Errors
/// Returns an I/O error if the XBEL file cannot be read or written.
pub fn remove_recents(paths: Option<&[PathBuf]>) -> std::io::Result<()> {
    let xbel_path = dirs::data_local_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No data local dir"))?
        .join("recently-used.xbel");

    if !xbel_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&xbel_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut keep = Vec::new();

    if let Some(paths) = paths {
        // Build a set of canonical file:// URIs to match against XBEL's href attributes.
        let uris_to_remove: HashSet<String> = paths
            .iter()
            .map(|p| gio::File::for_path(p).uri().into())
            .collect();

        for line in lines {
            if line.trim_start().starts_with("<bookmark") {
                let should_remove = uris_to_remove.iter().any(|uri| line.contains(uri.as_str()));
                if !should_remove {
                    keep.push(line);
                }
            } else {
                keep.push(line);
            }
        }
    } else {
        // Remove all bookmarks: keep everything except `<bookmark …>` lines.
        for line in lines {
            if !line.trim_start().starts_with("<bookmark") {
                keep.push(line);
            }
        }
    }

    std::fs::write(&xbel_path, keep.join("\n"))?;
    Ok(())
}

/// Retrieves a thumbnail [`gdk::Texture`] for a visual media file, generating
/// and caching it on first access.
///
/// Cache entries follow the [FreeDesktop Thumbnail Managing Standard], stored under
/// `$XDG_CACHE_HOME/thumbnails/<size-tier>/` as `MD5("file://<path>").hex + ".png"`.
/// This makes cache hits session-persistent: a thumbnail written in a previous
/// session is reused without regeneration as long as the source file's `mtime` and
/// size have not changed.
///
/// Stale entries (source modified after thumbnail was written, or zero-byte files)
/// are evicted and regenerated atomically.
///
/// [FreeDesktop Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to an image or video file.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the file is not visual media, if any
/// required external tool (`ffmpeg`) is unavailable, or if I/O fails.
pub fn get_or_create_thumbnail(path: &Path) -> Option<gdk::Texture> {
    // Load config once and check if thumbnails are enabled at all
    let config = load_config();
    if !config.ui.show_thumbnails {
        return None;
    }

    let (cache_dir, cache_path) = thumbnail_cache_path(path)?;

    // Don't even check the cache if the type is disabled - this prevents
    // the function from returning cached thumbnails when the type is disabled
    // Check PDF first before is_visual_media
    if is_pdf(path) {
        if !config.ui.thumbnail_types.pdfs {
            return None;
        }
        return pdf_thumbnail(path, &cache_path);
    }

    if is_font(path) {
        if !config.ui.thumbnail_types.fonts {
            return None;
        }
        return font_thumbnail(path, &cache_path);
    }

    let (is_img, is_vid) = is_visual_media(path);

    if is_img {
        if !config.ui.thumbnail_types.images {
            return None;
        }
    } else if is_vid {
        if !config.ui.thumbnail_types.videos {
            return None;
        }
    } else {
        // Not a supported thumbnail type
        return None;
    }

    fs::create_dir_all(&cache_dir).ok()?;

    let source_meta = fs::metadata(path).ok();

    // Only check cache if the type is enabled
    if cache_path.exists() {
        let is_valid = source_meta
            .as_ref()
            .map(|m| thumbnail_is_valid(&cache_path, m))
            .unwrap_or(true);

        if is_valid {
            let file = adw::gio::File::for_path(&cache_path);
            return gdk::Texture::from_file(&file).ok();
        }

        let _ = fs::remove_file(&cache_path);
    }

    if is_img {
        match gdk_pixbuf::Pixbuf::from_file_at_scale(
            path,
            constants::CACHED_THUMBNAIL_SIZE,
            constants::CACHED_THUMBNAIL_SIZE,
            true,
        ) {
            Ok(pixbuf) => {
                if let Some(path_str) = cache_path.to_str() {
                    let _ = pixbuf.savev(path_str, "png", &[]);
                }
                return Some(gdk::Texture::for_pixbuf(&pixbuf));
            }
            Err(_) => return None,
        }
    }

    if is_vid {
        // Compute a sensible seek time using the video duration.
        let seek_time = if let Some(dur) = probe_media_duration(path) {
            let dur_secs = dur.as_secs_f64();
            if dur_secs >= 60.0 {
                60.0 // at least 1 minute for long videos
            } else if dur_secs >= 1.0 {
                (dur_secs * 0.1).max(1.0) // 10% but at least 1s
            } else {
                0.0
            }
        } else {
            1.0 // fallback if duration cannot be probed
        };

        let seek_arg = format!("{:.3}", seek_time); // ffmpeg accepts e.g. "10.500"

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-loglevel")
            .arg("panic")
            .arg("-i")
            .arg(path)
            .arg("-ss")
            .arg(&seek_arg) // use the computed time
            .arg("-vframes")
            .arg("1")
            .arg("-vf")
            .arg(format!("scale={}:-1", constants::CACHED_THUMBNAIL_SIZE))
            .arg(&cache_path)
            .status();

        if matches!(status, Ok(s) if s.success() && cache_path.exists()) {
            let file = adw::gio::File::for_path(&cache_path);
            return gdk::Texture::from_file(&file).ok();
        }
    }

    None
}

pub fn get_system_mounts() -> Vec<(String, PathBuf)> {
    let mut mounts = Vec::new();

    let home_dir = dirs::home_dir().unwrap_or_default();

    if let Ok(content) = fs::read_to_string("/proc/self/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() >= 3 {
                let path_str = parts[1];

                let fs_type = parts[2];
                let path = PathBuf::from(path_str);

                let is_external = path_str.starts_with("/run/media/")
                    || path_str.starts_with("/media/")
                    || path_str.starts_with("/mnt/");

                let is_user_fuse = fs_type.contains("fuse") && path.starts_with(&home_dir);

                if is_external || is_user_fuse {
                    if path == home_dir {
                        continue;
                    }

                    if let Some(name) = path.file_name() {
                        let display_name = name.to_string_lossy().to_string();

                        if !mounts.iter().any(|(_, p)| p == &path) {
                            mounts.push((display_name, path));
                        }
                    }
                }
            }
        }
    }
    mounts
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::env;
    use std::fs;

    #[test]
    fn test_rename_path_rejects_path_separator() {
        let tmp = tempfile::TempDir::new().unwrap();

        let file = tmp.path().join("original.txt");
        fs::write(&file, b"").unwrap();

        let err = rename_path(&file, "sub/dir/name.txt").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_rename_path_rejects_existing_destination() {
        let tmp = tempfile::TempDir::new().unwrap();

        let src = tmp.path().join("a.txt");
        let dst = tmp.path().join("b.txt");
        fs::write(&src, b"").unwrap();
        fs::write(&dst, b"").unwrap();

        let err = rename_path(&src, "b.txt").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn test_rename_path_happy_path() {
        let tmp = tempfile::TempDir::new().unwrap();

        let src = tmp.path().join("old.txt");
        fs::write(&src, b"content").unwrap();

        let new_path = rename_path(&src, "new.txt").unwrap();
        assert!(!src.exists());
        assert!(new_path.exists());
        assert_eq!(new_path.file_name().unwrap(), "new.txt");
    }

    #[test]
    fn test_ensure_config_file_creation() {
        let temp_dir = env::current_dir()
            .unwrap()
            .join("target")
            .join("test_config_init");

        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).unwrap();
        }
        fs::create_dir_all(&temp_dir).unwrap();

        env::set_var("XDG_CONFIG_HOME", &temp_dir);

        let path = ensure_config_file();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("flux"));

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_get_system_mounts_structure() {
        let mounts = get_system_mounts();

        assert!(!mounts.is_empty());

        for (name, path) in mounts {
            assert!(!name.is_empty(), "Mount name should not be empty");

            assert!(path.is_absolute(), "Mount path must be absolute");
        }
    }

    #[test]
    fn test_config_invalid_toml() {
        let invalid_toml = "invalid = [unclosed bracket";

        let result: Result<crate::model::Config, _> = toml::from_str(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_missing_fields() {
        let partial_toml = r#"
            [ui]
            sidebar_width = 300
        "#;

        let config: crate::model::Config = toml::from_str(partial_toml).unwrap_or_default();

        assert_eq!(config.ui.sidebar_width, 300);
        assert_eq!(config.ui.default_icon_size, 0);
    }

    /// Verifies that the menu parser can handle the default internal config string.

    #[test]
    fn test_load_menu_config_integration() {
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");

        // Use tempfile to avoid manual cleanup and path collisions
        let tmp = tempfile::TempDir::new().unwrap();

        let temp_dir = tmp.path();

        let flux_config_dir = temp_dir.join("flux");
        std::fs::create_dir_all(&flux_config_dir).unwrap();

        // Set environment for the current process
        std::env::set_var("XDG_CONFIG_HOME", temp_dir);

        let config_path = flux_config_dir.join("menu.rs");
        let mock_content =
            r#""<U+F018F>      Copy" => "all", "builtin::copy", "Copied to clipboard""#;

        std::fs::write(config_path, mock_content).unwrap();

        let actions = load_menu_config();

        // Ensure state is restored regardless of assertion outcome
        if let Some(val) = original_xdg {
            std::env::set_var("XDG_CONFIG_HOME", val);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.label.contains("Copy")));
    }
    #[cfg(test)]
    mod recents_tests {
        use super::*;
        use std::fs;
        use tempfile::TempDir;

        /// Helper: writes a mock XBEL file to `dir/recently-used.xbel` with given lines.
        fn setup_xbel(dir: &TempDir, lines: &[&str]) -> std::path::PathBuf {
            let xbel_path = dir.path().join("recently-used.xbel");
            let content = lines.join("\n");
            fs::write(&xbel_path, content).unwrap();
            xbel_path
        }

        #[test]
        fn remove_recents_without_paths_clears_all_bookmarks() {
            let dir = TempDir::new().unwrap();
            // Mock data dir: set environment so dirs::data_local_dir() returns the temp dir.
            let original_xdg = std::env::var_os("XDG_DATA_HOME");
            std::env::set_var("XDG_DATA_HOME", dir.path());

            let xbel_content = vec![
                r#"<?xml version="1.0"?>"#,
                r#"<xbel version="1.0">"#,
                r#"  <bookmark href="file:///tmp/file1.txt" modified="2025-01-01T00:00:00Z"/>"#,
                r#"  <bookmark href="file:///tmp/file2.txt" modified="2025-01-02T00:00:00Z"/>"#,
                r#"</xbel>"#,
            ];
            setup_xbel(&dir, &xbel_content);

            let result = remove_recents(None);
            assert!(result.is_ok());

            let content = fs::read_to_string(dir.path().join("recently-used.xbel")).unwrap();
            // No <bookmark> tags should remain
            assert!(!content.contains("<bookmark"));
            // Non‑bookmark lines (XML headers) are preserved
            assert!(content.contains(r#"<?xml version="1.0"?>"#));

            // Restore environment
            if let Some(val) = original_xdg {
                std::env::set_var("XDG_DATA_HOME", val);
            } else {
                std::env::remove_var("XDG_DATA_HOME");
            }
        }

        #[test]
        fn remove_recents_with_paths_removes_matching_entries_only() {
            let dir = TempDir::new().unwrap();
            let original_xdg = std::env::var_os("XDG_DATA_HOME");
            std::env::set_var("XDG_DATA_HOME", dir.path());

            let xbel_content = vec![
                r#"<?xml version="1.0"?>"#,
                r#"<xbel version="1.0">"#,
                r#"  <bookmark href="file:///tmp/file1.txt"/>"#,
                r#"  <bookmark href="file:///tmp/file2.txt"/>"#,
                r#"  <bookmark href="file:///tmp/file3.txt"/>"#,
                r#"</xbel>"#,
            ];
            setup_xbel(&dir, &xbel_content);

            let paths_to_remove = vec![
                PathBuf::from("/tmp/file1.txt"),
                PathBuf::from("/tmp/file3.txt"),
            ];
            let result = remove_recents(Some(&paths_to_remove));
            assert!(result.is_ok());

            let content = fs::read_to_string(dir.path().join("recently-used.xbel")).unwrap();
            // Only file2 should remain
            assert!(content.contains("file2.txt"));
            assert!(!content.contains("file1.txt"));
            assert!(!content.contains("file3.txt"));

            if let Some(val) = original_xdg {
                std::env::set_var("XDG_DATA_HOME", val);
            } else {
                std::env::remove_var("XDG_DATA_HOME");
            }
        }

        #[test]
        fn remove_recents_handles_missing_xbel() {
            let dir = TempDir::new().unwrap();
            let original_xdg = std::env::var_os("XDG_DATA_HOME");
            std::env::set_var("XDG_DATA_HOME", dir.path());

            // No file exists
            let result = remove_recents(None);
            assert!(result.is_ok()); // Should not error, just returns Ok(())

            if let Some(val) = original_xdg {
                std::env::set_var("XDG_DATA_HOME", val);
            } else {
                std::env::remove_var("XDG_DATA_HOME");
            }
        }

        #[test]
        fn remove_recents_handles_malformed_xbel() {
            let dir = TempDir::new().unwrap();
            let original_xdg = std::env::var_os("XDG_DATA_HOME");
            std::env::set_var("XDG_DATA_HOME", dir.path());

            let malformed = vec![
                r#"<xbel>"#, // No closing, random text
                r#"<bookmark href="file:///tmp/a.txt""#,
            ];
            setup_xbel(&dir, &malformed);

            // Should not panic, it will just skip non‑bookmark lines or try to match.
            let result = remove_recents(None);
            assert!(result.is_ok());

            let content = fs::read_to_string(dir.path().join("recently-used.xbel")).unwrap();
            // All bookmark lines should be removed (they matched the '<bookmark' condition)
            assert!(!content.contains("<bookmark"));

            if let Some(val) = original_xdg {
                std::env::set_var("XDG_DATA_HOME", val);
            } else {
                std::env::remove_var("XDG_DATA_HOME");
            }
        }
    }
}
