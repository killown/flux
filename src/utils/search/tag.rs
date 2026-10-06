/// Parses a tag query from the search input.
///
/// Syntax:
/// - `:tag:work` or `:t:work` → tags: ["work"]
/// - `:tag:work,urgent` or `#work,urgent` → tags: ["work", "urgent"]
///
/// Returns `Some((tags, remaining_query))` or `None`.
pub fn parse_tag_filter(query: &str) -> Option<(Vec<String>, String)> {
    let query_trim = query.trim();
    if query_trim.is_empty() {
        return None;
    }

    let parts: Vec<&str> = query_trim.split_whitespace().collect();

    for (i, part) in parts.iter().enumerate() {
        let raw_tags = part
            .strip_prefix(":tag:")
            .or_else(|| part.strip_prefix(":t:"))
            .or_else(|| part.strip_prefix('#'));

        if let Some(tag_str) = raw_tags {
            let tags: Vec<String> = tag_str
                .split(',')
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty())
                .collect();

            if !tags.is_empty() {
                let rest = parts
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, s)| *s)
                    .collect::<Vec<_>>()
                    .join(" ");
                return Some((tags, rest));
            }
        }
    }

    None
}
