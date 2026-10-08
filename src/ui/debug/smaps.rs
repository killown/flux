use std::fs;

#[derive(Debug, Clone, Default)]
pub(super) struct SmapsRegion {
    pub addr: String,
    pub perms: String,
    pub path: String,
    pub size_kb: u64,
    pub rss_kb: u64,
    pub pss_kb: u64,
    pub private_dirty_kb: u64,
    pub category: String,
}

pub(super) fn classify_path(path: &str) -> String {
    let raw = path.trim();
    if raw == "[heap]" {
        return "heap (brk)".to_string();
    }
    if raw.starts_with("[stack") {
        return "thread stack".to_string();
    }
    if raw.is_empty() || raw == "[anon]" {
        return "anon mmap (mimalloc / glibc arena / raw buf)".to_string();
    }
    if raw.starts_with("/dev/shm") || raw.contains("memfd") {
        return "shared mem (wayland/shm)".to_string();
    }
    let lower = raw.to_lowercase();
    if lower.contains(".cache/thumbnails") {
        return "xdg thumbnail cache".to_string();
    }
    if lower.contains("state.db") {
        return "sqlite state.db (mmap/wal)".to_string();
    }
    if lower.contains("icon-theme.cache") || lower.contains("/icons/") {
        return "icon theme / assets".to_string();
    }
    if lower.ends_with(".ttf")
        || lower.ends_with(".otf")
        || lower.ends_with(".woff")
        || lower.ends_with(".woff2")
        || lower.contains("font")
        || lower.contains("pango")
    {
        return "font / pango".to_string();
    }
    if lower.contains("vulkan")
        || lower.contains("libgl")
        || lower.contains("mesa")
        || lower.contains("nvidia")
        || lower.contains("radeon")
        || lower.contains("/dri/")
    {
        return "gpu driver / gl / vulkan".to_string();
    }
    if lower.contains("libgtk")
        || lower.contains("libadw")
        || lower.contains("libgsk")
        || lower.contains("libgdk")
        || lower.contains("libglib")
    {
        return "gtk / adw libs".to_string();
    }
    if lower.contains("libgio") || lower.contains("gvfs") {
        return "gio / gvfs".to_string();
    }
    if lower.contains("/flux") {
        return "flux binary".to_string();
    }
    "other mapped files".to_string()
}

pub(super) fn read_detailed_smaps() -> Vec<SmapsRegion> {
    let content = fs::read_to_string("/proc/self/smaps").unwrap_or_default();
    let mut mapped_regions = Vec::with_capacity(512);
    let mut current_region = SmapsRegion::default();
    let mut parsing_region = false;

    for line in content.lines() {
        if line.contains('-') && line.contains(' ') && !line.starts_with(' ') {
            if parsing_region {
                current_region.category = classify_path(&current_region.path);
                mapped_regions.push(current_region.clone());
            }
            current_region = SmapsRegion::default();
            parsing_region = true;

            let mut tokens = line.splitn(6, ' ');
            current_region.addr = tokens.next().unwrap_or("").to_string();
            current_region.perms = tokens.next().unwrap_or("").to_string();
            let _ = tokens.next();
            let _ = tokens.next();
            let _ = tokens.next();
            current_region.path = tokens.next().unwrap_or("").trim().to_string();
            continue;
        }

        if !parsing_region {
            continue;
        }

        if let Some(val) = line.strip_prefix("Size:") {
            current_region.size_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("Rss:") {
            current_region.rss_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("Pss:") {
            current_region.pss_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("Private_Dirty:") {
            current_region.private_dirty_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        }
    }

    if parsing_region {
        current_region.category = classify_path(&current_region.path);
        mapped_regions.push(current_region);
    }

    mapped_regions
}
