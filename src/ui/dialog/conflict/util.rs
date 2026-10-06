use adw::prelude::*;

// ─── Utility helpers ──────────────────────────────────────────────────────────

pub(super) fn resolve_icon_name(path: &std::path::Path) -> String {
    let gio_file = gtk::gio::File::for_path(path);
    if let Ok(info) = gio_file.query_info(
        "standard::icon",
        gtk::gio::FileQueryInfoFlags::NONE,
        gtk::gio::Cancellable::NONE,
    ) {
        if let Some(icon) = info.icon() {
            if let Some(themed) = icon.downcast_ref::<gtk::gio::ThemedIcon>() {
                if let Some(name) = themed.names().first() {
                    return name.to_string();
                }
            }
        }
    }

    if path.is_dir() {
        return "folder-symbolic".to_string();
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "avif" => "image-x-generic-symbolic",
        "mp4" | "mkv" | "avi" | "mov" | "webm" => "video-x-generic-symbolic",
        "mp3" | "flac" | "ogg" | "wav" | "aac" => "audio-x-generic-symbolic",
        "pdf" => "x-office-document-symbolic",
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" | "rar" => "package-x-generic-symbolic",
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "go" | "sh" => "text-x-script-symbolic",
        _ => "text-x-generic-symbolic",
    }
    .to_string()
}

pub(super) fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub(super) fn format_mtime(meta: &std::fs::Metadata) -> String {
    use std::time::SystemTime;

    let Ok(modified) = meta.modified() else {
        return String::new();
    };
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else {
        return crate::i18n::tr("just now");
    };

    let secs = elapsed.as_secs();
    if secs < 60 {
        crate::i18n::tr("just now")
    } else if secs < 3_600 {
        let m = secs / 60;
        if m == 1 {
            crate::i18n::tr("1 minute ago")
        } else {
            format!("{} {}", m, crate::i18n::tr("minutes ago"))
        }
    } else if secs < 86_400 {
        let h = secs / 3_600;
        if h == 1 {
            crate::i18n::tr("1 hour ago")
        } else {
            format!("{} {}", h, crate::i18n::tr("hours ago"))
        }
    } else {
        let d = secs / 86_400;
        if d == 1 {
            crate::i18n::tr("Yesterday")
        } else if d < 30 {
            format!("{} {}", d, crate::i18n::tr("days ago"))
        } else {
            let secs_since_epoch = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let days = secs_since_epoch / 86_400;
            let (y, m, d) = days_to_ymd(days);
            format!("{:04}-{:02}-{:02}", y, m, d)
        }
    }
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let jd = days + 2_440_588;
    let l = jd + 68_569;
    let n = 4 * l / 146_097;
    let l = l - (146_097 * n).div_ceil(4);
    let i = 4_000 * (l + 1) / 1_461_001;
    let l = l - 1_461 * i / 4 + 31;
    let j = 80 * l / 2_447;
    let d = l - 2_447 * j / 80;
    let l = j / 11;
    let m = j + 2 - 12 * l;
    let y = 100 * (n - 49) + i + l;
    (y, m, d)
}
