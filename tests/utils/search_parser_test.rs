use flux::utils::search::{
    parse_content_search_query, parse_size_filter, parse_tag_filter, SizeOp,
};

#[test]
fn test_parse_size_filter_units() {
    let (op, _) = parse_size_filter(">10k").unwrap();
    assert_eq!(op, SizeOp::Gt(10 * 1024));

    let (op, _) = parse_size_filter(">10kb").unwrap();
    assert_eq!(op, SizeOp::Gt(10 * 1024));

    let (op, _) = parse_size_filter(">10kib").unwrap();
    assert_eq!(op, SizeOp::Gt(10 * 1024));

    let (op, _) = parse_size_filter("<5m").unwrap();
    assert_eq!(op, SizeOp::Lt(5 * 1024 * 1024));

    let (op, _) = parse_size_filter("<5mb").unwrap();
    assert_eq!(op, SizeOp::Lt(5 * 1024 * 1024));

    let (op, _) = parse_size_filter("<5mib").unwrap();
    assert_eq!(op, SizeOp::Lt(5 * 1024 * 1024));

    let (op, _) = parse_size_filter(">2g").unwrap();
    assert_eq!(op, SizeOp::Gt(2 * 1024 * 1024 * 1024));

    let (op, _) = parse_size_filter(">1tb").unwrap();
    assert_eq!(op, SizeOp::Gt(1024 * 1024 * 1024 * 1024));
}

#[test]
fn test_parse_size_filter_range_variations() {
    let (op, rest) = parse_size_filter("1mb..500mb my_file.iso").unwrap();
    assert_eq!(op, SizeOp::Range(1024 * 1024, 500 * 1024 * 1024));
    assert_eq!(rest, "my_file.iso");

    let (op, rest) = parse_size_filter("100kb..1mb").unwrap();
    assert_eq!(op, SizeOp::Range(100 * 1024, 1024 * 1024));
    assert_eq!(rest, "");
}

#[test]
fn test_parse_size_filter_malformed_inputs() {
    assert!(parse_size_filter(">").is_none());
    assert!(parse_size_filter("<").is_none());
    assert!(parse_size_filter("..").is_none());
    assert!(parse_size_filter(">abc").is_none());
    assert!(parse_size_filter("10mb..").is_none());
    assert!(parse_size_filter("..50mb").is_none());
    assert!(parse_size_filter(">100xyz").is_none());
}

#[test]
fn test_parse_tag_filter_syntax_variations() {
    let (tags, rest) = parse_tag_filter("#rust,code test_file").unwrap();
    assert_eq!(tags, vec!["rust", "code"]);
    assert_eq!(rest, "test_file");

    let (tags, rest) = parse_tag_filter(":tag:docs,archived").unwrap();
    assert_eq!(tags, vec!["docs", "archived"]);
    assert_eq!(rest, "");

    let (tags, rest) = parse_tag_filter(":t:todo,urgent urgent.txt").unwrap();
    assert_eq!(tags, vec!["todo", "urgent"]);
    assert_eq!(rest, "urgent.txt");
}

#[test]
fn test_parse_tag_filter_empty_and_noise() {
    assert!(parse_tag_filter("#").is_none());
    assert!(parse_tag_filter(":tag:").is_none());
    assert!(parse_tag_filter(":t:").is_none());
    assert!(parse_tag_filter("just a normal search").is_none());
    assert!(parse_tag_filter("").is_none());
    assert!(parse_tag_filter("   ").is_none());
}

#[test]
fn test_parse_content_search_query_guards() {
    let (term, ext) = parse_content_search_query(":function").unwrap();
    assert_eq!(term, "function");
    assert_eq!(ext, None);

    let (term, ext) = parse_content_search_query(":.rs:impl").unwrap();
    assert_eq!(term, "impl");
    assert_eq!(ext, Some("rs".to_string()));

    assert!(parse_content_search_query(":tag:work").is_none());
    assert!(parse_content_search_query(":t:work").is_none());

    assert!(parse_content_search_query(":ab").is_none());
    assert!(parse_content_search_query(":.rs:ab").is_none());
}
