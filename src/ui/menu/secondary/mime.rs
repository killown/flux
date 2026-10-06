/// Splits "image/png" or "application-wpoffice" into ("image", "png") or ("application", "wpoffice").
#[inline]
pub(super) fn split_mime(mime: &str) -> (&str, &str) {
    if let Some(idx) = mime.find('/') {
        (&mime[..idx], &mime[idx + 1..])
    } else if let Some(idx) = mime.find('-') {
        (&mime[..idx], &mime[idx + 1..])
    } else {
        (mime, "")
    }
}

/// Replaces characters that are illegal in filenames with `_`.
#[inline]
pub(super) fn sanitise_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '+' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}
