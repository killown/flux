use flux::utils::xml::{escape_attr, unescape_attr};

#[test]
fn escapes_the_five_predefined_entities() {
    assert_eq!(
        escape_attr(r#"a&b<c>d"e'f"#),
        "a&amp;b&lt;c&gt;d&quot;e&apos;f"
    );
}

#[test]
fn unescapes_the_five_predefined_entities() {
    assert_eq!(
        unescape_attr("a&amp;b&lt;c&gt;d&quot;e&apos;f"),
        r#"a&b<c>d"e'f"#
    );
}

#[test]
fn ampersand_is_decoded_last_so_nothing_decodes_twice() {
    assert_eq!(unescape_attr("&amp;lt;"), "&lt;");
    assert_eq!(unescape_attr("&amp;amp;"), "&amp;");
}

#[test]
fn matches_what_glib_writes_for_a_real_uri() {
    let raw = "file:///home/me/test1%20&%20test2's%20%22q%22.txt";
    let stored = "file:///home/me/test1%20&amp;%20test2&apos;s%20%22q%22.txt";
    assert_eq!(escape_attr(raw), stored);
    assert_eq!(unescape_attr(stored), raw);
}

#[test]
fn numeric_references_are_decoded() {
    assert_eq!(unescape_attr("&#39;&#x27;&#X26;"), "''&");
}

#[test]
fn unknown_or_unterminated_entities_are_left_alone() {
    assert_eq!(unescape_attr("R&D"), "R&D");
    assert_eq!(unescape_attr("a &bogus; b"), "a &bogus; b");
    assert_eq!(unescape_attr("trailing &"), "trailing &");
    assert_eq!(unescape_attr("&#;&#xZZ;"), "&#;&#xZZ;");
}

#[test]
fn round_trips_arbitrary_text() {
    let s = r#"&amp; <tag attr="x"> 'q' &lt;"#;
    assert_eq!(unescape_attr(&escape_attr(s)), s);
}
