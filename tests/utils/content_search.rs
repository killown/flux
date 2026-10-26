use flux::utils::search::parse_content_search_query;

#[test]
fn test_parse_content_search_query() {
    assert_eq!(
        parse_content_search_query(":hello"),
        Some(("hello".to_string(), None))
    );
    assert_eq!(parse_content_search_query(":hi"), None);
    assert_eq!(parse_content_search_query(":.rs"), None);
    assert_eq!(
        parse_content_search_query(":.rs:hello"),
        Some(("hello".to_string(), Some("rs".to_string())))
    );
    assert_eq!(
        parse_content_search_query(":.rs,py:hello"),
        Some(("hello".to_string(), Some("rs,py".to_string())))
    );
    assert_eq!(
        parse_content_search_query(":rs:hello"),
        Some(("hello".to_string(), None))
    );
    assert_eq!(parse_content_search_query(":.rs:hi"), None);
    assert_eq!(
        parse_content_search_query(":. :hello"),
        Some(("hello".to_string(), None))
    );
    assert_eq!(
        parse_content_search_query(":.:hello"),
        Some(("hello".to_string(), None))
    );
}
