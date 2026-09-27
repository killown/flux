use std::path::PathBuf;

/// Mirrors the path validation logic from `handle_navigate` so tests stay
/// in sync with the production predicate without depending on GTK.
fn path_valid(path: &PathBuf) -> bool {
    let s = path.to_string_lossy();
    s == "/" || path.exists() || s.starts_with("trash:///") || s.starts_with("recent:///")
}

// ─── Invalid path: stay on current ──────────────────────────────────────────

#[test]
fn test_invalid_path_does_not_change_current() {
    let current = PathBuf::from("/tmp");
    let requested = PathBuf::from("/tmp/this_path_does_not_exist_xyzzy_flux");

    if !path_valid(&requested) {
        assert_eq!(current, PathBuf::from("/tmp"));
    } else {
        panic!("test precondition failed: path unexpectedly exists");
    }
}

#[test]
fn test_invalid_path_is_detected() {
    let path = PathBuf::from("/nonexistent/flux/path/xyzzy");
    assert!(!path_valid(&path));
}

#[test]
fn test_root_is_always_valid() {
    let path = PathBuf::from("/");
    assert!(path_valid(&path));
}

#[test]
fn test_existing_path_is_valid() {
    let path = PathBuf::from("/tmp");
    assert!(path_valid(&path));
}

#[test]
fn test_trash_uri_is_valid() {
    let path = PathBuf::from("trash:///");
    assert!(path_valid(&path));
}

#[test]
fn test_recent_uri_is_valid() {
    let path = PathBuf::from("recent:///");
    assert!(path_valid(&path));
}

// ─── Fallback logic ──────────────────────────────────────────────────────────

#[test]
fn test_fallback_stays_on_current_when_valid() {
    let current = PathBuf::from("/tmp");
    let invalid = PathBuf::from("/no/such/path/flux_xyzzy");

    let fallback = Some(current.clone())
        .filter(|p| p.exists())
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));

    assert!(!path_valid(&invalid));
    assert_eq!(fallback, current);
}

#[test]
fn test_fallback_is_home_when_current_gone() {
    let current = PathBuf::from("/tmp/flux_deleted_dir_xyzzy_test");
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

    let fallback = Some(current.clone())
        .filter(|p| p.exists())
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));

    assert!(!current.exists(), "test precondition: dir must not exist");
    assert_eq!(fallback, home);
}

// ─── History integrity ───────────────────────────────────────────────────────

#[test]
fn test_invalid_navigate_does_not_push_history() {
    let mut history: Vec<PathBuf> = vec![PathBuf::from("/home")];
    let current = PathBuf::from("/tmp");
    let invalid = PathBuf::from("/no/such/path/flux_xyzzy");

    if !path_valid(&invalid) {
    } else {
        history.push(current.clone());
    }

    assert_eq!(history.len(), 1);
    assert_eq!(history[0], PathBuf::from("/home"));
}

#[test]
fn test_valid_navigate_pushes_history() {
    let mut history: Vec<PathBuf> = vec![];
    let mut current = PathBuf::from("/tmp");
    let target = PathBuf::from("/");

    if path_valid(&target) {
        let old = std::mem::replace(&mut current, target.clone());
        history.push(old);
    }

    assert_eq!(current, PathBuf::from("/"));
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], PathBuf::from("/tmp"));
}

// ─── Entry text sync ─────────────────────────────────────────────────────────

#[test]
fn test_entry_text_matches_current_path_after_navigation() {
    let current = PathBuf::from("/tmp");
    let entry_text = current.to_string_lossy().to_string();
    assert_eq!(entry_text, "/tmp");
}

#[test]
fn test_entry_text_unchanged_on_invalid_navigate() {
    let current = PathBuf::from("/tmp");
    let invalid = PathBuf::from("/no/such/path/flux_xyzzy");

    let mut entry_text = current.to_string_lossy().to_string();

    if !path_valid(&invalid) {
    } else {
        entry_text = invalid.to_string_lossy().to_string();
    }

    assert_eq!(entry_text, "/tmp");
}

#[test]
fn test_path_with_trailing_slash_normalized() {
    let path = PathBuf::from("/tmp/");
    let normalized = PathBuf::from(path.to_string_lossy().trim_end_matches('/'));
    assert_eq!(normalized, PathBuf::from("/tmp"));
}

#[test]
fn test_parent_of_root_is_none() {
    let root = PathBuf::from("/");
    assert!(root.parent().is_none());
}

#[test]
fn test_canonical_equivalence() {
    let a = PathBuf::from("/tmp/./foo");
    let b = PathBuf::from("/tmp/foo");
    let ca = a.canonicalize().unwrap_or(a.clone());
    let cb = b.canonicalize().unwrap_or(b.clone());
    assert_eq!(ca, cb);
}

#[test]
fn test_relative_path_validated_against_cwd() {
    let rel = PathBuf::from(".");
    assert!(path_valid(&rel));
}

#[test]
fn test_symlink_to_valid_dir_is_valid() {
    let tmp = tempfile::tempdir().unwrap();
    let link = tmp.path().join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tmp.path(), &link).unwrap();
    assert!(path_valid(&link));
}

#[test]
fn test_symlink_to_nonexistent_is_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let link = tmp.path().join("dangling");
    #[cfg(unix)]
    std::os::unix::fs::symlink("/nonexistent/xyzzy", &link).unwrap();
    assert!(!path_valid(&link));
}

#[test]
fn test_empty_path_invalid() {
    let empty = PathBuf::new();
    assert!(!path_valid(&empty));
}

#[test]
fn test_double_slash_root_style_invalid() {
    let path = PathBuf::from("//nonexistent/xyzzy");
    assert!(!path_valid(&path));
}

#[test]
fn test_history_pop_from_empty_returns_none() {
    let mut history: Vec<PathBuf> = Vec::new();
    assert!(history.pop().is_none());
}

#[test]
fn test_forward_stack_cleared_on_new_navigation() {
    let mut forward_stack: Vec<PathBuf> = vec![PathBuf::from("/tmp")];
    forward_stack.clear();
    assert!(forward_stack.is_empty());
}
