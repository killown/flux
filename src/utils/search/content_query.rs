/// Shortest content-search term accepted, counted in bytes, not characters.
pub const MIN_CONTENT_TERM_LEN: usize = 3;

pub const CONTENT_QUERY_SYNTAXES: &[(&str, &str)] = &[
    (":TERM", "Search all files for TERM"),
    (
        ":.EXT:TERM",
        "Search only files with extension EXT for TERM",
    ),
];

/// Parses a content‑search query string.
///
/// Syntax:
/// - `:term` → searches all files for `term`
/// - `:.ext:term` → searches only files with extension `ext`
///
/// Returns `Some((term, Some(ext)))` or `None` if the query is invalid.
pub fn parse_content_search_query(query: &str) -> Option<(String, Option<String>)> {
    let query_lc = query.to_lowercase();
    if !query_lc.starts_with(':') {
        return None;
    }
    let rest = &query_lc[1..];

    if rest.starts_with("tag:") || rest.starts_with("t:") {
        return None;
    }

    if let Some(second_col_pos) = rest.find(':') {
        let ext_part = rest[..second_col_pos].trim();
        let term_part = rest[second_col_pos + 1..].trim();

        if term_part.len() < MIN_CONTENT_TERM_LEN {
            return None;
        }
        let ext_filter = if ext_part.starts_with('.') && ext_part.len() > 1 {
            Some(ext_part[1..].trim().to_string())
        } else {
            None
        };
        Some((term_part.to_string(), ext_filter))
    } else {
        let term = rest.trim();
        if term.starts_with('.') || term.len() < MIN_CONTENT_TERM_LEN {
            return None;
        }
        Some((term.to_string(), None))
    }
}
