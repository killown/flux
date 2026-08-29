use globset::{Glob, GlobSet, GlobSetBuilder};

/// Shell-glob pattern matching for session-scoped file-type filtering.
///
/// Supports `*` (any sequence of characters, including empty) and `?`
/// (exactly one character). Both `pattern` and `name` must be pre-lowercased
/// by the caller, this function performs no case normalization itself.
///
/// Uses a bottom-up dynamic programming approach to handle degenerate inputs
/// (e.g. multiple consecutive `*` wildcards) without exponential backtracking.
///
/// # Examples
/// ```
/// use flux::utils::glob::glob_match,
/// assert!(glob_match("*.rs",   "main.rs")),
/// assert!(glob_match("a*.py",  "app.py")),
/// assert!(glob_match("a?b",    "axb")),
/// assert!(!glob_match("*.rs",  "main.py")),
/// assert!(glob_match("*",      "anything")),
/// assert!(glob_match("",       "")),
/// assert!(!glob_match("",      "x")),
/// ```
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

/// Expands a MIME-category shorthand into concrete extension glob patterns.
///
/// Recognised shorthands (case-insensitive):
/// - `image/*`  → all image extensions
/// - `video/*`  → all video extensions  
/// - `audio/*`  → all audio extensions
/// - `font/*`   → all font extensions
/// - `doc/*`    → common document extensions
///
/// Any other pattern is returned as-is in a single-element Vec.
pub fn expand_mime_category(pattern: &str) -> Vec<String> {
    let exts: &[&str] = match pattern.to_lowercase().as_str() {
        "image/*" => &[
            "*.jpg", "*.jpeg", "*.png", "*.gif", "*.webp", "*.avif", "*.heic", "*.heif", "*.bmp",
            "*.tiff", "*.tif", "*.jxl", "*.svg",
        ],
        "video/*" => &[
            "*.mp4", "*.mkv", "*.webm", "*.avi", "*.mov", "*.flv", "*.wmv", "*.m4v", "*.mpg",
            "*.mpeg", "*.ts", "*.ogv",
        ],
        "audio/*" => &[
            "*.mp3", "*.flac", "*.ogg", "*.opus", "*.wav", "*.aac", "*.m4a", "*.wma", "*.aiff",
        ],
        "font/*" => &["*.ttf", "*.otf", "*.woff", "*.woff2", "*.ttc"],
        "doc/*" => &[
            "*.pdf", "*.doc", "*.docx", "*.odt", "*.txt", "*.md", "*.epub",
        ],
        _ => {
            // Real MIME type (e.g. "application/zip", "image/png"), look up
            // matching globs from the system MIME database.
            if pattern.contains('/') {
                let mime = pattern.to_lowercase();
                for path in &["/usr/share/mime/globs2", "/usr/share/mime/globs"] {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let globs: Vec<String> = content
                            .lines()
                            .filter(|l| !l.starts_with('#'))
                            .filter_map(|l| {
                                // globs2: "weight:mime:glob[:flags]"  globs: "mime:glob"
                                let cols: Vec<&str> = l.split(':').collect();
                                // globs2 col[0] is weight (numeric), col[1]/col[2] are mime/glob
                                let (mime_part, glob_part) = match cols.len() {
                                    len if len >= 3 => (cols[1], cols[2]),
                                    2 => (cols[0], cols[1]),
                                    _ => return None,
                                };
                                if mime_part == mime {
                                    Some(glob_part.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !globs.is_empty() {
                            return globs;
                        }
                    }
                }
            }
            return vec![pattern.to_string()];
        }
    };
    exts.iter().map(|s| s.to_string()).collect()
}
