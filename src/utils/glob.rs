use globset::{Glob, GlobSet, GlobSetBuilder};

/// Shell-glob pattern matching for session-scoped file-type filtering.
///
/// Supports `*` (any sequence of characters, including empty) and `?`
/// (exactly one character). Both `pattern` and `name` must be pre-lowercased
/// by the caller, this function performs no case normalization itself.
///
/// Uses a bottom-up dynamic programming approach to handle degenerate inputs
/// (e.g. multiple consecutive `*` wildcards) without exponential backtracking.
#[allow(dead_code)]
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let (pl, nl) = (p.len(), n.len());

    let mut dp = vec![vec![false; nl + 1]; pl + 1];
    dp[0][0] = true;

    // A run of leading `*` wildcards matches the empty string.
    for i in 1..=pl {
        if p[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }

    for i in 1..=pl {
        for j in 1..=nl {
            dp[i][j] = match p[i - 1] {
                // `*` matches zero chars (dp[i-1][j]) or one more char (dp[i][j-1]).
                '*' => dp[i - 1][j] || dp[i][j - 1],
                '?' => dp[i - 1][j - 1],
                c => dp[i - 1][j - 1] && c == n[j - 1],
            };
        }
    }
    dp[pl][nl]
}

/// Compiles a list of glob patterns into a precomputed matcher.
///
/// Use this instead of `glob_match` for filtering many files against the
/// same set of patterns. Returns `None` if no valid patterns are provided.
#[allow(dead_code)]
pub fn compile_patterns(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().ok().filter(|s| !s.is_empty())
}

/// Returns a reference to the global MIME database, parsed once from
/// `/usr/share/mime/globs2` (preferred) or `/usr/share/mime/globs`.
///
/// Keys are lowercase MIME types (`"application/zip"`), values are the
/// glob patterns registered for that type (`["*.zip", "*.zipx"]`).
fn mime_db() -> &'static std::collections::HashMap<String, Vec<String>> {
    static DB: std::sync::OnceLock<std::collections::HashMap<String, Vec<String>>> =
        std::sync::OnceLock::new();

    DB.get_or_init(|| {
        let mut map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for path in &["/usr/share/mime/globs2", "/usr/share/mime/globs"] {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    if line.starts_with('#') || line.is_empty() {
                        continue;
                    }
                    // globs2: "weight:mime/type:*.ext"
                    // globs:  "mime/type:*.ext"
                    let cols: Vec<&str> = line.split(':').collect();
                    let (mime, glob) = if cols.len() >= 3 {
                        (cols[cols.len() - 2].to_string(), cols[cols.len() - 1])
                    } else if cols.len() == 2 {
                        (cols[0].to_string(), cols[1])
                    } else {
                        continue;
                    };
                    if mime.contains('/') {
                        map.entry(mime.to_lowercase())
                            .or_default()
                            .push(glob.to_string());
                    }
                }
                // Prefer globs2, stop after first successful parse.
                if !map.is_empty() {
                    break;
                }
            }
        }
        map
    })
}

/// Expands a MIME-category shorthand or real MIME type into concrete glob
/// patterns. Built-in shorthands are resolved from static slices, real MIME
/// types (anything containing `/`) are resolved from the system database
/// cached in a `OnceLock` - O(1) after the first call.
///
/// Built-in shorthands (case-insensitive):
/// - `image/*`, `video/*`, `audio/*`, `font/*`, `doc/*`
///
/// Any unrecognised pattern without a `/` is returned as-is.
pub fn expand_mime_category(pattern: &str) -> Vec<String> {
    let lc = pattern.to_lowercase();

    // Built-in shorthands - static, zero allocation.
    let shorthand: Option<&[&str]> = match lc.as_str() {
        "image/*" => Some(&[
            "*.jpg", "*.jpeg", "*.png", "*.gif", "*.webp", "*.avif", "*.heic", "*.heif", "*.bmp",
            "*.tiff", "*.tif", "*.jxl", "*.svg",
        ]),
        "video/*" => Some(&[
            "*.mp4", "*.mkv", "*.webm", "*.avi", "*.mov", "*.flv", "*.wmv", "*.m4v", "*.mpg",
            "*.mpeg", "*.ts", "*.ogv",
        ]),
        "audio/*" => Some(&[
            "*.mp3", "*.flac", "*.ogg", "*.opus", "*.wav", "*.aac", "*.m4a", "*.wma", "*.aiff",
        ]),
        "font/*" => Some(&["*.ttf", "*.otf", "*.woff", "*.woff2", "*.ttc"]),
        "doc/*" => Some(&[
            "*.pdf", "*.doc", "*.docx", "*.odt", "*.txt", "*.md", "*.epub",
        ]),
        _ => None,
    };
    if let Some(exts) = shorthand {
        return exts.iter().map(|s| s.to_string()).collect();
    }

    if lc.contains('/') {
        if let Some(globs) = mime_db().get(&lc) {
            return globs.clone();
        }
        // Unknown MIME type: return a pattern that matches nothing so the
        // filter doesn't silently pass everything through.
        return vec![format!("__unknown_mime__{}", lc)];
    }

    // Plain glob (*.zip, *.rs) - pass through as-is.
    vec![pattern.to_string()]
}
