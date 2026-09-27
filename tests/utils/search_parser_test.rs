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

#[test]
fn test_parse_size_filter_whitespace_boundaries() {
    assert!(parse_size_filter("").is_none());
    assert!(parse_size_filter("   ").is_none());
    let (op, _) = parse_size_filter("  >10MB  ").unwrap();
    assert_eq!(op, SizeOp::Gt(10 * 1024 * 1024));
}

#[test]
fn test_parse_size_filter_decimal_bytes_rejected() {
    assert!(parse_size_filter(">1.5MB").is_none());
}

#[test]
fn test_parse_size_filter_zero_value() {
    let (op, _) = parse_size_filter(">0MB").unwrap();
    assert_eq!(op, SizeOp::Gt(0));
}

#[test]
fn test_parse_size_filter_case_insensitive_units() {
    let (a, _) = parse_size_filter(">10mb").unwrap();
    let (b, _) = parse_size_filter(">10MB").unwrap();
    let (c, _) = parse_size_filter(">10Mb").unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn test_parse_size_filter_range_reversed_is_still_parsed() {
    let (op, _) = parse_size_filter("50MB..10MB").unwrap();
    assert_eq!(op, SizeOp::Range(50 * 1024 * 1024, 10 * 1024 * 1024));
}

#[test]
fn test_parse_size_filter_multiple_operators_uses_first() {
    let (op, rest) = parse_size_filter(">1MB <2MB").unwrap();
    assert_eq!(op, SizeOp::Gt(1024 * 1024));
    assert_eq!(rest, "<2mb");
}

#[test]
fn test_parse_tag_filter_preserves_case() {
    let (tags, _) = parse_tag_filter("#Rust,CODING").unwrap();
    assert_eq!(tags, vec!["rust", "coding"]);
}

#[test]
fn test_parse_tag_filter_deduplicates_when_called() {
    let (tags, _) = parse_tag_filter("#rust,rust").unwrap();
    assert_eq!(tags.len(), 2);
}

#[test]
fn test_parse_tag_filter_numeric_tags() {
    let (tags, _) = parse_tag_filter("#2026,2027").unwrap();
    assert_eq!(tags, vec!["2026", "2027"]);
}

#[test]
fn test_parse_tag_filter_single_char() {
    let (tags, _) = parse_tag_filter("#a").unwrap();
    assert_eq!(tags, vec!["a"]);
}

#[test]
fn test_parse_tag_filter_with_dots_and_dashes() {
    let (tags, _) = parse_tag_filter("#work-in-progress,project.v2").unwrap();
    assert_eq!(tags, vec!["work-in-progress", "project.v2"]);
}

#[test]
fn test_parse_content_search_multi_ext_whitespace() {
    let (term, ext) = parse_content_search_query(":.rs, py, toml:term").unwrap();
    assert_eq!(ext, Some("rs, py, toml".to_string()));
    assert_eq!(term, "term");
}

#[test]
fn test_parse_content_search_unicode_term() {
    let (term, ext) = parse_content_search_query(":café").unwrap();
    assert_eq!(term, "café");
    assert_eq!(ext, None);
}

#[test]
fn test_parse_content_search_term_exactly_three() {
    let (term, _) = parse_content_search_query(":abc").unwrap();
    assert_eq!(term, "abc");
}

#[test]
fn test_parse_content_search_term_two_chars_rejected() {
    assert!(parse_content_search_query(":ab").is_none());
}

#[test]
fn test_parse_content_search_only_colon() {
    assert!(parse_content_search_query(":").is_none());
}

#[test]
fn test_parse_content_search_no_colon_prefix() {
    assert!(parse_content_search_query("hello").is_none());
}

#[test]
fn test_parse_content_search_tag_collision_guards() {
    assert!(parse_content_search_query(":tag:").is_none());
    assert!(parse_content_search_query(":t:").is_none());
}

#[test]
fn parse_size_filter_lowercase_gb() {
    let (op, _) = parse_size_filter(">2gb").unwrap();
    assert_eq!(op, SizeOp::Gt(2 * 1024 * 1024 * 1024));
}

#[test]
fn parse_size_filter_mixed_case_kb() {
    let (op, _) = parse_size_filter(">10Kb").unwrap();
    assert_eq!(op, SizeOp::Gt(10 * 1024));
}

#[test]
fn parse_size_filter_range_with_same_value() {
    let (op, _) = parse_size_filter("1MB..1MB").unwrap();
    assert_eq!(op, SizeOp::Range(1024 * 1024, 1024 * 1024));
}

#[test]
fn parse_size_filter_rest_preserved_verbatim() {
    let (_, rest) = parse_size_filter(">1MB hello world").unwrap();
    assert_eq!(rest, "hello world");
}

#[test]
fn parse_tag_filter_many_tags() {
    let (tags, _) = parse_tag_filter("#a,b,c,d,e").unwrap();
    assert_eq!(tags.len(), 5);
}

#[test]
fn parse_tag_filter_leading_whitespace_in_query() {
    let (tags, _) = parse_tag_filter("   #foo,bar").unwrap();
    assert_eq!(tags, vec!["foo", "bar"]);
}

#[test]
fn parse_content_search_query_max_reasonable_length() {
    let term = "a".repeat(1000);
    let query = format!(":{}", term);
    let (t, e) = parse_content_search_query(&query).unwrap();
    assert_eq!(t.len(), 1000);
    assert!(e.is_none());
}

#[test]
fn parse_content_search_query_with_unicode_extension() {
    let (term, ext) = parse_content_search_query(":.文档:内容").unwrap();
    assert_eq!(term, "内容");
    assert_eq!(ext, Some("文档".to_string()));
}

#[test]
fn parse_size_filter_tb_lowercase() {
    let (op, _) = parse_size_filter("<1tb").unwrap();
    assert_eq!(op, SizeOp::Lt(1024 * 1024 * 1024 * 1024));
}

#[test]
fn parse_size_filter_naive_range_rejected() {
    assert!(parse_size_filter("10MB-50MB").is_none());
}
