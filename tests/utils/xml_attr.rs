//! `utils::xml::attr_value` on `<bookmark>` start tags as GLib writes them.

use flux::utils::xml::attr_value;
use gio::prelude::FileExt;
use std::path::PathBuf;

/// Exactly what `g_bookmark_file_to_data` writes for
/// `/home/me/Tom & Jerry's "q".txt`.
const GLIB_LINE: &str = r#"<bookmark href="file:///home/me/Tom%20&amp;%20Jerry&apos;s%20%22q%22.txt" added="2024-05-01T10:00:00Z" modified="2024-05-02T11:30:00Z" visited="2024-05-03T12:45:00Z">"#;

#[test]
fn decodes_entities_in_href() {
    assert_eq!(
        attr_value(GLIB_LINE, "href").as_deref(),
        Some("file:///home/me/Tom%20&%20Jerry's%20%22q%22.txt")
    );
}

#[test]
fn decoded_href_resolves_to_the_real_path() {
    let href = attr_value(GLIB_LINE, "href").unwrap();
    let path = gio::File::for_uri(&href).path().unwrap();
    assert_eq!(path, PathBuf::from(r#"/home/me/Tom & Jerry's "q".txt"#));
}

#[test]
fn decodes_a_bare_ampersand_in_a_folder_name() {
    let tag = r#"<bookmark href="file:///home/me/R&amp;D" added="a" modified="b">"#;
    assert_eq!(
        attr_value(tag, "href").as_deref(),
        Some("file:///home/me/R&D")
    );
}

#[test]
fn leaves_plain_values_alone() {
    let tag = r#"<bookmark href="file:///tmp/a.txt" added="x" modified="y">"#;
    assert_eq!(
        attr_value(tag, "href").as_deref(),
        Some("file:///tmp/a.txt")
    );
    assert_eq!(attr_value(tag, "added").as_deref(), Some("x"));
    assert_eq!(attr_value(tag, "modified").as_deref(), Some("y"));
}

#[test]
fn reads_each_timestamp_independently() {
    assert_eq!(
        attr_value(GLIB_LINE, "modified").as_deref(),
        Some("2024-05-02T11:30:00Z")
    );
    assert_eq!(
        attr_value(GLIB_LINE, "added").as_deref(),
        Some("2024-05-01T10:00:00Z")
    );
}

#[test]
fn returns_none_for_missing_or_unterminated_attributes() {
    assert_eq!(attr_value(GLIB_LINE, "nope"), None);
    assert_eq!(attr_value(r#"<bookmark href="file:///tmp/a"#, "href"), None);
}
