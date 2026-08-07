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
    if let Some(second_col_pos) = rest.find(':') {
        let ext_part = rest[..second_col_pos].trim();
        let term_part = rest[second_col_pos + 1..].trim();
        if term_part.len() < 3 {
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
        if term.starts_with('.') || term.len() < 3 {
            return None;
        }
        Some((term.to_string(), None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_search_query() {
        assert_eq!(
            parse_content_search_query(":hello"),
            Some(("hello".to_string(), None))
        );
        assert_eq!(parse_content_search_query(":hi"), None);
        assert_eq!(parse_content_search_query(":.rs"), None);
        assert_eq!(
            parse_content_search_query(":.rs:hello"),
            Some(("hello".to_string(), Some("rs".to_string())))
        );
        assert_eq!(
            parse_content_search_query(":.rs,py:hello"),
            Some(("hello".to_string(), Some("rs,py".to_string())))
        );
        assert_eq!(
            parse_content_search_query(":rs:hello"),
            Some(("hello".to_string(), None))
        );
        assert_eq!(parse_content_search_query(":.rs:hi"), None);
        assert_eq!(
            parse_content_search_query(":. :hello"),
            Some(("hello".to_string(), None))
        );
        assert_eq!(
            parse_content_search_query(":.:hello"),
            Some(("hello".to_string(), None))
        );
    }
}
