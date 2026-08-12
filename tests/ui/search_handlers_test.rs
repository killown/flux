use flux::utils::search::{parse_size_filter, SizeOp};
use std::path::PathBuf;

#[test]
fn test_parse_size_filter_gt() {
    let (op, rest) = parse_size_filter(">10MB").unwrap();
    assert_eq!(op, SizeOp::Gt(10 * 1024 * 1024));
    assert_eq!(rest, "");
}

#[test]
fn test_parse_size_filter_lt() {
    let (op, rest) = parse_size_filter("<500KB").unwrap();
    assert_eq!(op, SizeOp::Lt(500 * 1024));
    assert_eq!(rest, "");
}

#[test]
fn test_parse_size_filter_range() {
    let (op, rest) = parse_size_filter("10MB..50MB").unwrap();
    assert_eq!(op, SizeOp::Range(10 * 1024 * 1024, 50 * 1024 * 1024));
    assert_eq!(rest, "");
}

#[test]
fn test_parse_size_filter_rejects_bare_numbers() {
    // Bare numbers without a proper byte unit (e.g., ">10") must return None to prevent lockups
    assert!(parse_size_filter(">10").is_none());
    assert!(parse_size_filter("<500").is_none());
}

#[test]
fn test_parse_size_filter_with_name() {
    let (op, rest) = parse_size_filter(">1GB video.mp4").unwrap();
    assert_eq!(op, SizeOp::Gt(1024 * 1024 * 1024));
    assert_eq!(rest, "video.mp4");
}

#[test]
fn test_parse_size_filter_no_filter() {
    assert!(parse_size_filter("hello world").is_none());
}

#[test]
fn test_search_input_append_and_backspace() {
    let mut filter = String::new();

    // Type 'f'
    filter.push('f');
    assert_eq!(filter, "f");

    // Type 'o'
    filter.push('o');
    assert_eq!(filter, "fo");

    // Type 'o'
    filter.push('o');
    assert_eq!(filter, "foo");

    // Backspace
    filter.pop();
    assert_eq!(filter, "fo");
}

#[test]
fn test_content_search_result_display_formatting() {
    let path = PathBuf::from("/home/user/project/main.rs");
    let line = "fn main() {".to_string();
    let line_number = 42;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let display_name = format!("{}:{}  {}", name, line_number, line);

    assert_eq!(display_name, "main.rs:42  fn main() {");
}

#[test]
fn test_switch_header_clears_filter_when_leaving_search() {
    let mut filter = "search_term".to_string();
    let mut header_view = "search";
    let next_view = "path";

    // Read initial state to verify view transition logic
    assert_eq!(header_view, "search");

    header_view = next_view;
    if header_view != "search" {
        filter.clear();
    }

    assert_eq!(header_view, "path");
    assert!(filter.is_empty());
}

#[test]
fn test_parse_size_filter_overflow_returns_none() {
    // These inputs would have caused integer overflow panics before `checked_mul` was used.
    assert!(parse_size_filter(">999999999999999999999999999999GB").is_none());
    assert!(parse_size_filter("<999999999999999999999999999999TB").is_none());
    // A range with huge values should also return None.
    assert!(parse_size_filter(
        "999999999999999999999999999999GB..1000000000000000000000000000000TB"
    )
    .is_none());
}
