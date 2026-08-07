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
