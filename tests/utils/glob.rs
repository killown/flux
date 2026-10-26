use flux::utils::glob::glob_match;

#[test]
fn exact_match() {
    assert!(glob_match("foo.rs", "foo.rs"));
    assert!(!glob_match("foo.rs", "bar.rs"));
}

#[test]
fn star_suffix() {
    assert!(glob_match("*.py", "main.py"));
    assert!(glob_match("*.py", ".py"));
    assert!(!glob_match("*.py", "main.rs"));
}

#[test]
fn star_prefix() {
    assert!(glob_match("main*", "main.rs"));
    assert!(glob_match("main*", "main"));
    assert!(!glob_match("main*", "other.rs"));
}

#[test]
fn question_mark() {
    assert!(glob_match("a?b", "axb"));
    assert!(!glob_match("a?b", "ab"));
    assert!(!glob_match("a?b", "axxb"));
}

#[test]
fn star_only_matches_anything() {
    assert!(glob_match("*", "anything"));
    assert!(glob_match("*", ""));
}

#[test]
fn multiple_stars() {
    assert!(glob_match("a*b*c", "aXbYc"));
    assert!(glob_match("a**c", "ac"));
}

#[test]
fn combined() {
    assert!(glob_match("a*.py", "app.py"));
    assert!(glob_match("a*.py", "a.py"));
    assert!(!glob_match("a*.py", "b.py"));
}

#[test]
fn test_expand_mime_category_shorthands() {
    use flux::utils::glob::expand_mime_category;

    let images = expand_mime_category("image/*");
    assert!(images.contains(&"*.jpg".to_string()));
    assert!(images.contains(&"*.png".to_string()));
    assert!(images.contains(&"*.svg".to_string()));

    let videos = expand_mime_category("video/*");
    assert!(videos.contains(&"*.mp4".to_string()));
    assert!(videos.contains(&"*.mkv".to_string()));

    let docs = expand_mime_category("doc/*");
    assert!(docs.contains(&"*.pdf".to_string()));
    assert!(docs.contains(&"*.docx".to_string()));

    // Pass-through for plain globs
    assert_eq!(
        expand_mime_category("*.custom"),
        vec!["*.custom".to_string()]
    );
}

#[test]
fn empty_pattern_matches_empty_string() {
    assert!(glob_match("", ""));
    assert!(!glob_match("", "a"));
}

#[test]
fn star_matches_slash() {
    assert!(glob_match("*", "path/to/file"));
}

#[test]
fn question_does_not_match_empty() {
    assert!(!glob_match("?", ""));
}

#[test]
fn case_sensitive_by_default() {
    assert!(!glob_match("FOO", "foo"));
    assert!(!glob_match("foo", "FOO"));
}

#[test]
fn unicode_question_mark() {
    assert!(glob_match("a?b", "aèb"));
    assert!(glob_match("?", "ñ"));
}

#[test]
fn multiple_question_marks() {
    assert!(glob_match("???", "abc"));
    assert!(!glob_match("???", "ab"));
    assert!(!glob_match("???", "abcd"));
}

#[test]
fn star_between_questions() {
    assert!(glob_match("a?*?b", "aXYb"));
    assert!(glob_match("a?*?b", "axxxyyyb"));
}

#[test]
fn trailing_star_matches_empty_tail() {
    assert!(glob_match("file*", "file"));
}

#[test]
fn leading_star_matches_empty_head() {
    assert!(glob_match("*.txt", ".txt"));
}

#[test]
fn complex_extension_pattern() {
    assert!(glob_match("*.tar.gz", "archive.tar.gz"));
    assert!(!glob_match("*.tar.gz", "archive.gz"));
}

#[test]
fn pattern_with_internal_dot() {
    assert!(glob_match("a.b.c", "a.b.c"));
    assert!(!glob_match("a.b.c", "abc"));
}

#[test]
fn expand_mime_category_case_insensitive() {
    use flux::utils::glob::expand_mime_category;
    let upper = expand_mime_category("IMAGE/*");
    let lower = expand_mime_category("image/*");
    assert_eq!(upper, lower);
}

#[test]
fn glob_dot_prefix_optional() {
    assert!(glob_match("*file", "file"));
    assert!(glob_match("*file", "afile"));
}

#[test]
fn glob_extension_only() {
    assert!(glob_match("*.txt", "a.txt"));
    assert!(!glob_match("*.txt", "a.txtx"));
}

#[test]
fn glob_multi_dot_name() {
    assert!(glob_match("*.tar.*", "x.tar.gz"));
}

#[test]
fn glob_utf8_pattern() {
    assert!(glob_match("café", "café"));
    assert!(!glob_match("café", "cafe"));
}

#[test]
fn glob_empty_string_to_star() {
    assert!(glob_match("*", ""));
}

#[test]
fn glob_single_char_no_star() {
    assert!(glob_match("a", "a"));
    assert!(!glob_match("a", "ab"));
}

#[test]
fn glob_all_lowercase_ext() {
    assert!(glob_match("*.mp3", "song.mp3"));
    assert!(glob_match("*.mp3", "song.MP3".to_lowercase().as_str()));
}

#[test]
fn glob_question_at_start() {
    assert!(glob_match("?foo", "afoo"));
    assert!(!glob_match("?foo", "foo"));
}

#[test]
fn glob_question_in_middle() {
    assert!(glob_match("a?b?c", "axbyc"));
    assert!(!glob_match("a?b?c", "abc"));
}

#[test]
fn glob_alternating_star_question() {
    assert!(glob_match("*?*", "x"));
    assert!(glob_match("*?*", "xy"));
    assert!(!glob_match("*?*", ""));
}

#[test]
fn glob_matches_empty_pattern_against_empty() {
    assert!(glob_match("", ""));
}

#[test]
fn glob_double_star_collapses() {
    assert!(glob_match("**", "anything"));
    assert!(glob_match("**", ""));
}

#[test]
fn glob_long_repetition_no_exponential_blowup() {
    // A naive recursive matcher would time out here, the DP version is O(n*m).
    let pat = "*a*a*a*a*a*a*a*a*a*a*b";
    let name = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaac";
    // No 'b' in name, must return false quickly.
    assert!(!glob_match(pat, name));
}

#[test]
fn expand_mime_category_doc_includes_pdf() {
    use flux::utils::glob::expand_mime_category;
    assert!(expand_mime_category("doc/*").contains(&"*.pdf".to_string()));
}
