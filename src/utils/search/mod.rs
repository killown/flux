//! Search-query parsing and fuzzy matching helpers.

mod content_query;
mod doc;
mod fuzzy;
mod size;
mod tag;

pub use content_query::{parse_content_search_query, MIN_CONTENT_TERM_LEN};
#[allow(unused_imports)]
// used by `src/lib.rs` for `gen-docs`, not linked by the `flux-fm` binary alone
pub use doc::render_search_markdown;
pub use fuzzy::fuzzy_match;
pub use size::{parse_size_filter, SizeOp};
pub use tag::parse_tag_filter;

// Public helper kept for callers that pre-compile their pattern.
#[allow(unused_imports)]
pub use fuzzy::fuzzy_match_compiled;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_is_in_sync() {
        let generated = doc::render_search_markdown();
        let committed = std::fs::read_to_string("docs/search.md")
            .expect("docs/search.md is missing; create it from render_search_markdown()");
        assert_eq!(
            generated, committed,
            "\ndocs/search.md is stale.\n\
             Run `cargo run --bin gen-docs` to regenerate it."
        );
    }
}
