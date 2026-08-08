use flux::utils::glob::expand_mime_category;
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
fn expand_mime_category_image() {
    let patterns = expand_mime_category("image/*");
    assert!(patterns.contains(&"*.png".to_string()));
    assert!(patterns.contains(&"*.jpg".to_string()));
    assert!(!patterns.contains(&"*.mp4".to_string()));
}

#[test]
fn expand_mime_category_video() {
    let patterns = expand_mime_category("video/*");
    assert!(patterns.contains(&"*.mp4".to_string()));
    assert!(patterns.contains(&"*.mkv".to_string()));
    assert!(!patterns.contains(&"*.png".to_string()));
}

#[test]
fn expand_mime_category_unknown_passthrough() {
    let patterns = expand_mime_category("*.rs");
    assert_eq!(patterns, vec!["*.rs".to_string()]);
}

#[test]
fn expand_mime_category_is_case_insensitive() {
    let lower = expand_mime_category("image/*");
    let upper = expand_mime_category("IMAGE/*");
    assert_eq!(lower, upper);
}
