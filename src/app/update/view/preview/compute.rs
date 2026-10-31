//! Computes the content the peek-preview panel shows for one file.

use super::super::text_encoding;
use super::probe::{
    archive_listing, audio_info, build_info, dir_listing, hex_dump, is_texty_mime, load_image_full,
    pdf_summary, read_head, video_info,
};
use crate::ui::panels::preview::{
    binary, markdown, office, syntax, table, FileInfo, PreviewContent,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

// ─── Limits ───────────────────────────────────────────────────────────────────

const TEXT_HEAD_BYTES: usize = 64 * 1024;
const TEXT_HEAD_LINES: usize = 500;
const HEX_BYTES: usize = 512;
const MAX_LINE_CHARS: usize = 2000;
const MAX_HIGHLIGHT_RANGES: usize = 5000;
const PREVIEW_DEADLINE: Duration = Duration::from_secs(10);

const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "webp", "bmp", "tif", "tiff", "svg", "ico", "avif", "heic",
    "heif", "jxl", "xpm", "pnm", "tga", "icns",
];
const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv", "m4v", "mpg", "mpeg", "ts", "ogv",
];
const AUDIO_EXTS: &[&str] = &["mp3", "flac", "m4a", "ogg", "wav", "aac", "wma", "opus"];
const TABLE_EXTS: &[&str] = &["csv", "tsv", "tab"];
const MARKDOWN_EXTS: &[&str] = &["md", "markdown", "mdown", "mkd", "mkdn"];

// ─── Entry point ──────────────────────────────────────────────────────────────

pub(crate) fn compute_preview_guarded(path: &Path) -> PreviewContent {
    crate::hit!("compute_preview_guarded");
    use crate::i18n::tr;

    let (tx, rx) = std::sync::mpsc::channel();
    let owned = path.to_path_buf();
    let t0 = std::time::Instant::now();
    let spawned = std::thread::Builder::new()
        .name("flux-preview".into())
        .spawn(move || {
            let _ = tx.send(compute_preview(&owned));
        });
    if spawned.is_err() {
        return degraded(path, tr("Preview failed."));
    }

    match rx.recv_timeout(PREVIEW_DEADLINE) {
        Ok(content) => {
            let ms = t0.elapsed().as_millis();
            if ms > 200 {
                eprintln!("[flux][preview] compute took {ms} ms: {}", path.display());
            }
            content
        }
        Err(RecvTimeoutError::Timeout) => {
            eprintln!("[flux][preview] timed out: {}", path.display());
            degraded(path, tr("Preview timed out. Use Open to view this file."))
        }
        Err(RecvTimeoutError::Disconnected) => {
            eprintln!("[flux][preview] worker panicked: {}", path.display());
            degraded(path, tr("Preview failed. Use Open to view this file."))
        }
    }
}

/// Metadata card that touches no filesystem state, so it cannot hang itself.
fn degraded(path: &Path, note: String) -> PreviewContent {
    use crate::i18n::tr;

    let mut info = FileInfo {
        path: path.to_path_buf(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        ..Default::default()
    };
    info.extra.push((tr("Note"), note));
    PreviewContent::Metadata { info }
}

pub(crate) fn compute_preview(path: &Path) -> PreviewContent {
    crate::hit!("compute_preview");
    use crate::i18n::tr;

    let virtual_path = path
        .to_string_lossy()
        .starts_with(crate::infra::archive::ARCHIVE_URI);
    if !virtual_path && !path.exists() {
        return PreviewContent::Error(tr("File no longer exists."));
    }

    let mut info = build_info(path, virtual_path);

    let real_path: Option<PathBuf> = if virtual_path {
        if let Some((archive_path, inner)) =
            crate::infra::archive::parse_archive_uri(&path.to_string_lossy())
        {
            let extracted =
                crate::infra::archive::extract_archive_entry_for_playback(&archive_path, &inner);
            if extracted.is_some() {
                info.extra.push((
                    tr("Extracted from"),
                    archive_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ));
            }
            extracted
        } else {
            None
        }
    } else {
        Some(path.to_path_buf())
    };

    let Some(real_path) = real_path else {
        return PreviewContent::Metadata { info };
    };
    let path = real_path.as_path();

    if path.is_dir() {
        return dir_listing(path, info);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let ext = ext.as_str();

    let ts_source = ext == "ts" && !crate::utils::media::is_mpeg_ts(path);
    if ts_source {
        info.mime = "text/x-typescript".to_string();
    }

    if ext == "pdf" {
        return pdf_preview(path, info);
    }

    // Office documents: convert to PDF, then render that.
    if office::OFFICE_EXTS.contains(&ext) && office::soffice_binary().is_some() {
        if let Some(pdf_path) = office::convert_to_pdf(path) {
            info.extra.push((
                tr("Converted from"),
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ));
            return pdf_preview(&pdf_path, info);
        }
    }

    // Executables: icon plus embedded metadata.
    if let Some(icon) = binary::inspect(path, ext, &mut info) {
        return match icon {
            Some(tex) => PreviewContent::Image { tex, info },
            None => PreviewContent::Metadata { info },
        };
    }

    if crate::infra::archive::is_supported_archive(path) {
        if let Some(listing) = archive_listing(path, &mut info) {
            return listing;
        }
    }

    if TABLE_EXTS.contains(&ext) {
        if let Some(data) = table::parse(path, ext) {
            info.extra.push((tr("Rows"), data.rows.len().to_string()));
            info.extra
                .push((tr("Columns"), data.headers.len().to_string()));
            let delim_name = if data.delimiter == b'\t' {
                "Tab"
            } else {
                "Comma"
            };
            info.extra.push((tr("Delimiter"), delim_name.to_string()));
            return PreviewContent::Table { data, info };
        }
    }

    if IMAGE_EXTS.contains(&ext) || info.mime.starts_with("image/") {
        if let Some((tex, extra)) = load_image_full(path) {
            info.extra.extend(extra);
            return PreviewContent::Image { tex, info };
        }
    }

    if AUDIO_EXTS.contains(&ext) || info.mime.starts_with("audio/") {
        info.extra.extend(audio_info(path));
        let cover = crate::utils::media::generate_thumbnail_sync(path);
        return PreviewContent::Media {
            path: path.to_path_buf(),
            video: false,
            cover,
            info,
        };
    }

    if (VIDEO_EXTS.contains(&ext) && !ts_source) || info.mime.starts_with("video/") {
        info.extra.extend(video_info(path));
        return PreviewContent::Media {
            path: path.to_path_buf(),
            video: true,
            cover: None,
            info,
        };
    }

    let is_font = matches!(ext, "ttf" | "otf" | "woff" | "woff2" | "ttc")
        || info.mime.starts_with("font/")
        || info.mime == "application/x-font-ttf"
        || info.mime == "application/x-font-otf"
        || info.mime == "application/font-sfnt"
        || info.mime == "application/font-woff";
    if is_font {
        // The path is already a real on-disk file (extracted from the archive
        // above if this was a virtual path), so fontconfig can open it directly.
        if let Some(tex) = crate::utils::media::font_specimen(path, 1024) {
            return PreviewContent::Image { tex, info };
        }
    }

    // Thumbnailer: fonts, executables, RAW.
    if let Some(tex) = crate::utils::media::generate_thumbnail_sync(path) {
        return PreviewContent::Image { tex, info };
    }

    // Text, Markdown, or a hex dump.
    let Ok(head) = read_head(path, TEXT_HEAD_BYTES) else {
        return PreviewContent::Metadata { info };
    };
    let hit_cap = head.len() >= TEXT_HEAD_BYTES;
    let texty_mime = is_texty_mime(&info.mime);

    if let Some(decoded) = text_encoding::decode_text(&head, hit_cap, texty_mime) {
        let text = decoded.text;
        let total_lines = text.lines().count();
        let mut clamped = false;
        let shown: String = text
            .lines()
            .take(TEXT_HEAD_LINES)
            .map(|l| match l.char_indices().nth(MAX_LINE_CHARS) {
                Some((i, _)) => {
                    clamped = true;
                    format!("{}…", &l[..i])
                }
                None => l.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let truncated = total_lines > TEXT_HEAD_LINES || hit_cap || clamped;

        info.extra.push((tr("Encoding"), decoded.label.to_string()));
        info.extra.push((
            tr("Line endings"),
            if text.contains("\r\n") { "CRLF" } else { "LF" }.to_string(),
        ));
        info.extra.push((
            tr("Lines"),
            if truncated {
                format!("{total_lines}+")
            } else {
                total_lines.to_string()
            },
        ));
        if !truncated {
            info.extra
                .push((tr("Words"), text.split_whitespace().count().to_string()));
        }

        // Markdown takes priority over syntect for `.md` files.
        if MARKDOWN_EXTS.contains(&ext) && !truncated {
            let md = markdown::render(&shown);
            return PreviewContent::Markdown {
                rendered: md.text,
                styles: md.styles,
                source: shown,
                info,
            };
        }

        let cfg_on = crate::utils::load_config().ui.preview_syntax_highlighting;
        let (mut highlighted, language) = if cfg_on && !clamped {
            syntax::highlight_file(&shown, path)
        } else {
            (Vec::new(), None)
        };
        if highlighted.len() > MAX_HIGHLIGHT_RANGES {
            highlighted.clear();
        }
        if let Some(ref lang) = language {
            info.extra.push((tr("Language"), lang.clone()));
        }

        return PreviewContent::Text {
            head: shown,
            truncated,
            hex: false,
            info,
            highlighted,
            language,
        };
    }

    let n = head.len().min(HEX_BYTES);
    PreviewContent::Text {
        head: hex_dump(&head[..n]),
        truncated: info.size as usize > n,
        hex: true,
        info,
        highlighted: Vec::new(),
        language: None,
    }
}

fn pdf_preview(path: &Path, mut info: FileInfo) -> PreviewContent {
    crate::hit!("pdf_preview");
    use crate::i18n::tr;
    match pdf_summary(path) {
        Some((pages, w_pt, h_pt)) => {
            info.extra.push((tr("Pages"), pages.to_string()));
            info.extra.push((
                tr("Page size"),
                format!("{:.0} × {:.0} mm", w_pt / 72.0 * 25.4, h_pt / 72.0 * 25.4),
            ));
            PreviewContent::Pdf {
                pages,
                info,
                render_path: path.to_path_buf(),
            }
        }
        None => {
            info.extra.push((
                tr("Note"),
                tr("Could not open this PDF (encrypted or damaged)."),
            ));
            PreviewContent::Metadata { info }
        }
    }
}
