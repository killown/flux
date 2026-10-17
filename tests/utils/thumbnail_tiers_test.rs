use flux::utils::media::{find_cached_thumbnail, thumbnail_lookup_tiers, tier_for_size};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const HASH: &str = "0a2a69c0c26de7a222ec9e98f876e7f1";

fn put(root: &Path, tier: &str) {
    let dir = root.join(tier);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{HASH}.png")), b"png").unwrap();
}

fn found_tier(root: &Path, size: i32) -> Option<String> {
    find_cached_thumbnail(root, HASH, size).map(|(p, _)| {
        p.parent()
            .and_then(|d| d.file_name())
            .unwrap()
            .to_string_lossy()
            .into_owned()
    })
}

#[test]
fn tier_boundaries_match_the_spec_sizes() {
    assert_eq!(tier_for_size(1), "normal");
    assert_eq!(tier_for_size(128), "normal");
    assert_eq!(tier_for_size(129), "large");
    assert_eq!(tier_for_size(256), "large");
    assert_eq!(tier_for_size(257), "x-large");
    assert_eq!(tier_for_size(512), "x-large");
    assert_eq!(tier_for_size(513), "xx-large");
    assert_eq!(tier_for_size(768), "xx-large");
}

#[test]
fn tier_for_non_positive_size_is_normal() {
    assert_eq!(tier_for_size(0), "normal");
    assert_eq!(tier_for_size(-5), "normal");
}

#[test]
fn lookup_starts_at_own_tier_and_never_goes_smaller() {
    assert_eq!(
        thumbnail_lookup_tiers(100),
        ["normal", "large", "x-large", "xx-large"]
    );
    assert_eq!(
        thumbnail_lookup_tiers(256),
        ["large", "x-large", "xx-large"]
    );
    assert_eq!(thumbnail_lookup_tiers(400), ["x-large", "xx-large"]);
    assert_eq!(thumbnail_lookup_tiers(768), ["xx-large"]);
}

#[test]
fn lookup_never_includes_a_smaller_tier_than_requested() {
    let nominal = |tier: &str| match tier {
        "normal" => 128,
        "large" => 256,
        "x-large" => 512,
        _ => 1024,
    };
    for size in 1..=1024 {
        for tier in thumbnail_lookup_tiers(size) {
            assert!(nominal(tier) >= size, "{tier} is too small for {size}");
        }
    }
}

#[test]
fn smaller_tier_thumbnail_is_not_used_for_a_bigger_request() {
    let root = TempDir::new().unwrap();
    put(root.path(), "normal");
    assert!(find_cached_thumbnail(root.path(), HASH, 256).is_none());
    assert!(find_cached_thumbnail(root.path(), HASH, 512).is_none());
}

#[test]
fn own_tier_is_preferred_over_larger_tiers() {
    let root = TempDir::new().unwrap();
    put(root.path(), "xx-large");
    put(root.path(), "large");
    put(root.path(), "x-large");
    assert_eq!(found_tier(root.path(), 256).as_deref(), Some("large"));
}

#[test]
fn smallest_sufficient_larger_tier_is_used_when_own_is_missing() {
    let root = TempDir::new().unwrap();
    put(root.path(), "xx-large");
    put(root.path(), "x-large");
    assert_eq!(found_tier(root.path(), 256).as_deref(), Some("x-large"));
}

#[test]
fn larger_tier_satisfies_a_smaller_request() {
    let root = TempDir::new().unwrap();
    put(root.path(), "large");
    assert_eq!(found_tier(root.path(), 100).as_deref(), Some("large"));
}

#[test]
fn larger_request_is_not_satisfied_by_a_smaller_tier() {
    let root = TempDir::new().unwrap();
    put(root.path(), "large");
    assert!(find_cached_thumbnail(root.path(), HASH, 1024).is_none());
}

#[test]
fn missing_root_or_empty_cache_finds_nothing() {
    let root = TempDir::new().unwrap();
    assert!(find_cached_thumbnail(&root.path().join("nope"), HASH, 256).is_none());
    assert!(find_cached_thumbnail(root.path(), HASH, 256).is_none());
}

#[test]
fn a_directory_with_the_thumbnail_name_is_ignored() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join("large").join(format!("{HASH}.png"))).unwrap();
    assert!(find_cached_thumbnail(root.path(), HASH, 256).is_none());
}

#[test]
fn returned_metadata_describes_the_found_file() {
    let root = TempDir::new().unwrap();
    put(root.path(), "large");
    let (path, meta) = find_cached_thumbnail(root.path(), HASH, 256).unwrap();
    assert!(path.ends_with(format!("large/{HASH}.png")));
    assert_eq!(meta.len(), 3);
}
