//! Minimal XML attribute-value escaping for the hand-rolled XBEL handling.

/// Escapes `&`, `<`, `>`, `"` and `'` so `s` can sit inside a double-quoted
/// XML attribute, matching what GLib writes.
pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Decodes `&lt, &gt, &quot, &apos, &amp,` and numeric character references
/// (`&#39,`, `&#x27,`) in an attribute value.
pub fn unescape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest
            .find(';')
            .and_then(|semi| decode_entity(&rest[1..semi]).map(|c| (c, semi + 1)));
        match decoded {
            Some((c, consumed)) => {
                out.push(c);
                rest = &rest[consumed..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Resolves the text between `&` and `,` to a character, if it names one.
fn decode_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// Extracts the value of a named, double-quoted attribute from a single-line
/// XML start tag and decodes its entities.
///
/// Matches `name="value"` and returns the decoded value. GLib escapes `&` and
/// `'` in the `href` of a `<bookmark>`, so `Tom%20&amp;%20Jerry` comes back as
/// `Tom%20&%20Jerry`. Only meant for the flat attributes of XBEL `<bookmark>`
/// tags, which GLib always writes double-quoted and on one line.
pub fn attr_value(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(unescape_attr(&rest[..end]))
}
