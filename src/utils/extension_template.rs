use std::collections::HashMap;
use std::sync::OnceLock;

static EXT_MIME_CACHE: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Returns the system-registered MIME type for any extension using the FreeDesktop database.
pub fn lookup_system_extension_mime(ext: &str) -> Option<String> {
    if ext.is_empty() {
        return None;
    }
    let ext_lower = ext.to_ascii_lowercase();

    let table = EXT_MIME_CACHE.get_or_init(|| {
        let mut map = HashMap::with_capacity(4096);

        // Standard Freedesktop shared-mime-info globs locations
        let candidate_paths = [
            "/usr/share/mime/globs2",
            "/usr/local/share/mime/globs2",
            "/usr/share/mime/globs",
            "/usr/local/share/mime/globs",
        ];

        for path in &candidate_paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with('#') || line.is_empty() {
                        continue;
                    }
                    // globs2: "weight:mime/type:*.ext:flags" or "weight:mime/type:*.ext"
                    // globs:  "mime/type:*.ext"
                    let cols: Vec<&str> = line.split(':').collect();
                    let (mime, glob) = if cols.len() >= 3 {
                        (cols[1].trim(), cols[2].trim())
                    } else if cols.len() == 2 {
                        (cols[0].trim(), cols[1].trim())
                    } else {
                        continue;
                    };

                    if let Some(clean_ext) = glob.strip_prefix("*.") {
                        if !clean_ext.contains('*') && !clean_ext.contains('?') {
                            map.entry(clean_ext.to_ascii_lowercase())
                                .or_insert_with(|| mime.to_string());
                        }
                    }
                }
                if !map.is_empty() {
                    break;
                }
            }
        }

        // Add user-specific local definitions if present (~/.local/share/mime/globs2)
        if let Some(user_globs) = dirs::data_local_dir().map(|d| d.join("mime/globs2")) {
            if let Ok(content) = std::fs::read_to_string(user_globs) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with('#') || line.is_empty() {
                        continue;
                    }
                    let cols: Vec<&str> = line.split(':').collect();
                    if cols.len() >= 3 {
                        let mime = cols[1].trim();
                        let glob = cols[2].trim();
                        if let Some(clean_ext) = glob.strip_prefix("*.") {
                            if !clean_ext.contains('*') && !clean_ext.contains('?') {
                                map.insert(clean_ext.to_ascii_lowercase(), mime.to_string());
                            }
                        }
                    }
                }
            }
        }

        map
    });

    table.get(&ext_lower).cloned()
}
