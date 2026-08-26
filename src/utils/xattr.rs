use std::path::Path;

/// Standard FreeDesktop / Linux extended attribute namespace for file tags.
pub const XDG_TAGS_ATTR: &str = "user.xdg.tags";

/// Read and parse tags from the file's extended attributes.
///
/// Tags in `user.xdg.tags` are conventionally stored as comma-separated or
/// newline-separated UTF-8 strings.
pub fn read_tags<P: AsRef<Path>>(path: P) -> Vec<String> {
    let raw_bytes = match xattr::get(path.as_ref(), XDG_TAGS_ATTR) {
        Ok(Some(data)) => data,
        _ => return Vec::new(),
    };

    let content = match String::from_utf8(raw_bytes) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    content
        .split([',', '\n'])
        .map(|s| s.trim().trim_start_matches('#').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Write a list of tags to the file's extended attributes.
///
/// If `tags` is empty, the `user.xdg.tags` attribute is removed from the file.
pub fn write_tags<P: AsRef<Path>>(path: P, tags: &[String]) -> Result<(), std::io::Error> {
    let p = path.as_ref();

    let clean_tags: Vec<&str> = tags
        .iter()
        .map(|s| s.trim().trim_start_matches('#'))
        .filter(|s| !s.is_empty())
        .collect();

    if clean_tags.is_empty() {
        match xattr::remove(p, XDG_TAGS_ATTR) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    } else {
        let serialized = clean_tags.join(",");
        xattr::set(p, XDG_TAGS_ATTR, serialized.as_bytes())
    }
}
