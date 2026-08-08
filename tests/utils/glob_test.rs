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
