//! Per-file probes behind the preview: the Details card, PDF and image
//! loading, media tags, folder and archive listings, and raw byte reads.

use crate::domain::model::{FileInfo, FolderEntry, PreviewContent};
use std::path::{Path, PathBuf};

const IMAGE_MAX_DIM: i32 = 2560;
const LISTING_SCAN_CAP: usize = 20_000;
const FOLDER_GRID_MAX_ITEMS: usize = 2000;

// ─── Info ─────────────────────────────────────────────────────────────────────

pub(super) fn build_info(path: &Path, virtual_path: bool) -> FileInfo {
    crate::hit!("build_info");
    use crate::i18n::tr;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut info = FileInfo {
        path: path.to_path_buf(),
        name,
        ..Default::default()
    };

    if virtual_path {
        info.mime = crate::utils::media::get_mime_type(path);
        return info;
    }

    let is_dir = path.is_dir();
    info.mime = if is_dir {
        "inode/directory".to_string()
    } else {
        crate::utils::media::get_mime_type(path)
    };

    if let Ok(meta) = std::fs::metadata(path) {
        info.size = if is_dir { 0 } else { meta.len() };
        info.mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o7777;
        info.extra.push((
            tr("Permissions"),
            format!("{} ({:o})", mode_string(mode), mode),
        ));
    }

    if let Ok(target) = std::fs::read_link(path) {
        info.extra
            .push((tr("Symlink to"), target.to_string_lossy().into_owned()));
    }
    info
}

fn mode_string(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        s.push(if bits & 4 != 0 { 'r' } else { '-' });
        s.push(if bits & 2 != 0 { 'w' } else { '-' });
        s.push(if bits & 1 != 0 { 'x' } else { '-' });
    }
    s
}

// ─── PDF ──────────────────────────────────────────────────────────────────────

pub(super) fn pdf_summary(path: &Path) -> Option<(usize, f64, f64)> {
    crate::hit!("pdf_summary");
    let doc = poppler::PopplerDocument::new_from_file(path, None).ok()?;
    let pages = doc.get_n_pages();
    if pages == 0 {
        return None;
    }
    let (w, h) = doc.get_page(0)?.get_size();
    Some((pages, w, h))
}

// ─── Images ───────────────────────────────────────────────────────────────────

pub(super) fn load_image_full(path: &Path) -> Option<(gtk::gdk::Texture, Vec<(String, String)>)> {
    crate::hit!("load_image_full");
    use crate::i18n::tr;

    let (format, w, h) = gdk_pixbuf::Pixbuf::file_info(path)?;
    let pixbuf = if w.max(h) > IMAGE_MAX_DIM {
        gdk_pixbuf::Pixbuf::from_file_at_scale(path, IMAGE_MAX_DIM, IMAGE_MAX_DIM, true).ok()?
    } else {
        gdk_pixbuf::Pixbuf::from_file(path).ok()?
    };
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);

    let extra = vec![
        (tr("Dimensions"), format!("{w} × {h} px")),
        (
            tr("Format"),
            format.name().map(|n| n.to_uppercase()).unwrap_or_default(),
        ),
    ];
    Some((gtk::gdk::Texture::for_pixbuf(&pixbuf), extra))
}

// ─── Audio / video ────────────────────────────────────────────────────────────

pub(super) fn audio_info(path: &Path) -> Vec<(String, String)> {
    crate::hit!("audio_info");
    use crate::i18n::tr;
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let mut rows = Vec::new();
    let Some(tagged) = Probe::open(path).ok().and_then(|p| p.read().ok()) else {
        return rows;
    };

    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        if let Some(v) = tag.title() {
            rows.push((tr("Title"), v.to_string()));
        }
        if let Some(v) = tag.artist() {
            rows.push((tr("Artist"), v.to_string()));
        }
        if let Some(v) = tag.album() {
            rows.push((tr("Album"), v.to_string()));
        }
        if let Some(v) = tag.genre() {
            rows.push((tr("Genre"), v.to_string()));
        }
        if let Some(v) = tag
            .get_string(lofty::tag::ItemKey::Year)
            .or_else(|| tag.get_string(lofty::tag::ItemKey::RecordingDate))
        {
            rows.push((tr("Year"), v.to_string()));
        }
    }

    let props = tagged.properties();
    rows.push((tr("Duration"), fmt_secs(props.duration().as_secs_f64())));
    if let Some(b) = props.audio_bitrate() {
        rows.push((tr("Bitrate"), format!("{b} kbps")));
    }
    if let Some(sr) = props.sample_rate() {
        rows.push((tr("Sample rate"), format!("{sr} Hz")));
    }
    if let Some(ch) = props.channels() {
        rows.push((tr("Channels"), ch.to_string()));
    }
    rows
}

pub(super) fn video_info(path: &Path) -> Vec<(String, String)> {
    crate::hit!("video_info");
    use crate::i18n::tr;
    use std::collections::HashMap;

    let mut rows = Vec::new();
    let Ok(out) = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,r_frame_rate:format=duration,bit_rate",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
    else {
        return rows;
    };
    if !out.status.success() {
        return rows;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let kv: HashMap<&str, &str> = text.lines().filter_map(|l| l.split_once('=')).collect();

    if let (Some(w), Some(h)) = (kv.get("width"), kv.get("height")) {
        rows.push((tr("Resolution"), format!("{w} × {h}")));
    }
    if let Some(c) = kv.get("codec_name") {
        rows.push((tr("Codec"), c.to_uppercase()));
    }
    if let Some(fps) = kv.get("r_frame_rate").and_then(|r| {
        let (n, d) = r.split_once('/')?;
        let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
        (d > 0.0).then_some(n / d)
    }) {
        rows.push((tr("Frame rate"), format!("{fps:.2} fps")));
    }
    if let Some(d) = kv.get("duration").and_then(|d| d.parse::<f64>().ok()) {
        rows.push((tr("Duration"), fmt_secs(d)));
    }
    if let Some(b) = kv.get("bit_rate").and_then(|b| b.parse::<f64>().ok()) {
        rows.push((tr("Bitrate"), format!("{:.0} kbps", b / 1000.0)));
    }
    rows
}

fn fmt_secs(secs: f64) -> String {
    let t = secs.max(0.0).round() as u64;
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

// ─── Folders / archives ───────────────────────────────────────────────────────

pub(super) fn dir_listing(path: &Path, mut info: FileInfo) -> PreviewContent {
    crate::hit!("dir_listing");
    use crate::i18n::tr;

    let Ok(rd) = std::fs::read_dir(path) else {
        return PreviewContent::Error(tr("Cannot read this folder."));
    };

    let show_hidden = crate::utils::load_config().ui.show_hidden_by_default;
    let mut entries: Vec<(String, bool, PathBuf)> = Vec::new();
    let mut scanned = 0usize;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        scanned += 1;
        if entries.len() < LISTING_SCAN_CAP {
            let is_dir = e
                .file_type()
                .is_ok_and(|t| t.is_dir() || (t.is_symlink() && e.path().is_dir()));
            entries.push((name, is_dir, e.path()));
        }
    }
    let dirs = entries.iter().filter(|e| e.1).count();
    let capped = scanned > entries.len();

    info.extra.push((
        tr("Items"),
        if capped {
            format!("{scanned}+")
        } else {
            scanned.to_string()
        },
    ));
    info.extra.push((tr("Folders"), dirs.to_string()));
    info.extra
        .push((tr("Files"), (entries.len() - dirs).to_string()));

    entries.sort_by_cached_key(|e| (!e.1, e.0.to_lowercase()));

    let more = entries.len().saturating_sub(FOLDER_GRID_MAX_ITEMS);
    let entries = entries
        .into_iter()
        .take(FOLDER_GRID_MAX_ITEMS)
        .map(|(name, is_dir, path)| {
            let meta = std::fs::metadata(&path).ok();
            let size = if is_dir {
                0
            } else {
                meta.as_ref().map_or(0, |m| m.len())
            };
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            FolderEntry {
                name,
                path,
                is_dir,
                size,
                mtime,
            }
        })
        .collect();

    PreviewContent::Folder {
        entries,
        more,
        info,
    }
}

pub(super) fn archive_listing(path: &Path, info: &mut FileInfo) -> Option<PreviewContent> {
    crate::hit!("archive_listing");
    use crate::i18n::tr;

    let mut entries = crate::infra::archive::list_archive_entries(path, "", None).ok()?;
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    info.extra
        .push((tr("Top-level entries"), entries.len().to_string()));

    // Archives reuse the folder grid: each entry becomes a cell addressed by its
    // `archive://` URI, so thumbnails and activation take the same paths as a folder.
    let more = entries.len().saturating_sub(FOLDER_GRID_MAX_ITEMS);
    let entries = entries
        .into_iter()
        .take(FOLDER_GRID_MAX_ITEMS)
        .map(|e| FolderEntry {
            path: crate::infra::archive::build_archive_uri(path, &e.inner_path),
            size: if e.is_dir { e.child_count } else { e.size },
            name: e.name,
            is_dir: e.is_dir,
            mtime: e.mtime,
        })
        .collect();

    Some(PreviewContent::Folder {
        entries,
        more,
        info: info.clone(),
    })
}

// ─── Text / hex ───────────────────────────────────────────────────────────────

pub(super) fn is_texty_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime.ends_with("+xml")
        || mime.ends_with("+json")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/yaml"
                | "application/toml"
                | "application/sql"
                | "application/x-shellscript"
                | "application/x-sh"
                | "application/x-desktop"
                | "application/x-perl"
                | "application/x-php"
                | "application/x-ruby"
                | "application/x-subrip"
        )
}

pub(super) fn hex_dump(bytes: &[u8]) -> String {
    crate::hit!("hex_dump");
    use std::fmt::Write;

    let mut out = String::with_capacity(bytes.len() * 4);
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "{:08x}  ", i * 16);
        for j in 0..16 {
            match chunk.get(j) {
                Some(b) => {
                    let _ = write!(out, "{b:02x} ");
                }
                None => out.push_str("   "),
            }
            if j == 7 {
                out.push(' ');
            }
        }
        out.push_str(" |");
        for &b in chunk {
            out.push(if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '.'
            });
        }
        out.push_str("|\n");
    }
    out
}

pub(super) fn read_head(path: &Path, cap: usize) -> std::io::Result<Vec<u8>> {
    crate::hit!("read_head");
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; cap];
    let mut total = 0;
    loop {
        match f.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total >= cap {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    buf.truncate(total);
    Ok(buf)
}
