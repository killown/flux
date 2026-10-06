//! Search-query parsing and fuzzy matching helpers.

mod content_query;
mod fuzzy;
mod size;
mod tag;

pub use content_query::parse_content_search_query;
pub use fuzzy::fuzzy_match;
pub use size::{parse_size_filter, SizeOp};
pub use tag::parse_tag_filter;

// Public helper kept for callers that pre-compile their pattern.
#[allow(unused_imports)]
pub use fuzzy::fuzzy_match_compiled;
