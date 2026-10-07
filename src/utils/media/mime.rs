use std::fs;
use std::io::Read;
use std::path::Path;

/// Returns `true` if `path` is a PDF file by extension (case-insensitive).
#[allow(dead_code)]
pub(super) fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Returns `true` if `path` is a font file by extension (case-insensitive).
#[allow(dead_code)]
pub(super) fn is_font(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "ttf" | "otf" | "woff" | "woff2" | "ttc"
        )
    })
}

/// Returns `true` when the theme's icon for `content_type` is a hit on a
/// container MIME that many unrelated extensions share (`.conf`, `.dat`,
/// `.cfg` all collapse to `application/xml` or `text/plain`), and the
/// extension itself is not that container's subtype. In that case the theme
/// icon is not evidence the theme can draw this extension, so generation
/// should run instead.
#[inline]
pub fn container_mime_masks_extension(ext: &str, content_type: &str) -> bool {
    if ext.is_empty() {
        return false;
    }

    let is_container = matches!(
        content_type,
        "application/xml"
            | "text/xml"
            | "application/json"
            | "application/octet-stream"
            | "application/x-zerosize"
            | "application/x-empty"
    );
    if !is_container {
        return false;
    }

    // If the extension literally IS the subtype (`.xml` → `xml`), the MIME is
    // specific and the theme icon is authoritative.
    let ext_is_subtype = content_type
        .split('/')
        .nth(1)
        .map(|s| s.trim_start_matches("x-").eq_ignore_ascii_case(ext))
        .unwrap_or(false);

    !ext_is_subtype
}

pub fn icon_gen_params() -> (bool, String, String, String, f64) {
    let (_, _, auto_gen, accent, body, font, font_size) = crate::utils::config::get_icon_config();
    (auto_gen, accent, body, font, font_size)
}

pub fn get_mime_type(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(mime) = guess_mime_from_extension(ext) {
                return mime;
            }
        }
        return "application/octet-stream".to_string();
    }

    if crate::services::network::is_network_uri(path) {
        if path_str.ends_with('/') {
            return "inode/directory".to_string();
        }

        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if filename.is_empty() {
            return "inode/directory".to_string();
        }

        let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), None);
        let ct = content_type.to_string();

        if ct == "inode/directory" {
            return guess_mime_from_extension(&filename)
                .unwrap_or_else(|| "application/octet-stream".to_string());
        }
        return ct;
    }

    if path.is_dir() {
        return "inode/directory".to_string();
    }

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let mut sniff_buffer = [0u8; 4096];
    let data_slice = if let Ok(mut file) = fs::File::open(path) {
        if let Ok(count) = file.read(&mut sniff_buffer) {
            Some(&sniff_buffer[..count])
        } else {
            None
        }
    } else {
        None
    };

    let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), data_slice);
    let ct = content_type.to_string();

    if ct == "inode/directory" && path.is_file() {
        return guess_mime_from_extension(&filename)
            .unwrap_or_else(|| "application/octet-stream".to_string());
    }

    ct
}

pub fn guess_mime_from_extension(filename: &str) -> Option<String> {
    let ext = std::path::Path::new(filename).extension()?.to_str()?;
    crate::utils::extension_template::lookup_system_extension_mime(ext)
}

pub fn is_visual_media(path: &Path) -> (bool, bool) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tiff"
            | "tif" | "jxl" | "svg",
        ) => (true, false),
        Some(
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts"
            | "ogv",
        ) => (false, true),
        // only hit GIO for unknown extensions
        _ => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let (ct, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
            (ct.starts_with("image/"), ct.starts_with("video/"))
        }
    }
}

pub fn is_audio_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "mp3" | "flac" | "m4a" | "ogg" | "wav"
        )
    })
}
