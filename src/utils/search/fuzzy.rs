use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use std::cell::RefCell;

thread_local! {
    static MATCHER_STATE: RefCell<(Matcher, Vec<char>)> = RefCell::new((
        Matcher::new(Config::DEFAULT),
        Vec::with_capacity(256),
    ));
}

/// Matches target string against a pre-parsed pattern using thread-local matcher and scratch buffer.
#[inline]
pub fn fuzzy_match_compiled(target: &str, pattern: &Pattern) -> bool {
    MATCHER_STATE.with(|state| {
        let (ref mut matcher, ref mut buf) = *state.borrow_mut();
        buf.clear();
        let utf32_target = Utf32Str::new(target, buf);
        pattern.score(utf32_target, matcher).is_some()
    })
}

/// High-performance fuzzy path matcher using Nucleo.
#[inline]
pub fn fuzzy_match(target: &str, pattern: &str) -> bool {
    let pattern_trimmed = pattern.trim();
    if pattern_trimmed.is_empty() {
        return true;
    }

    let parsed_pattern =
        Pattern::parse(pattern_trimmed, CaseMatching::Ignore, Normalization::Smart);

    fuzzy_match_compiled(target, &parsed_pattern)
}
