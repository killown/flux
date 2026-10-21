# Adding a Search Filter

The search bar dispatches on the query string in
`src/ui/search/filter.rs::handle_update_filter`. Each query syntax is a
parser that either matches and returns a modified view, or returns
`None` and lets the next parser try. Order of evaluation is load-bearing
and is not enforced by any test.

Read `src/ui/search/filter.rs` in full before starting.

## Checklist

1. **`src/utils/search/<syntax>.rs`** - new parser module, if the syntax
    is non-trivial. Follow the shape of `size.rs` or `tag.rs`: a
    `parse_<syntax>(query: &str) -> Option<(Payload, String)>` that
    returns the parsed payload and the remainder of the query. The
    parser is pure (no I/O, no allocator stress) and unit-tested in the
    same file under `#[cfg(test)]`.

2. **`src/utils/search/mod.rs`** - `mod <syntax>;` and
    `pub use <syntax>::parse_<syntax>;`.

3. **`src/ui/search/filter.rs`** - insert the guard in
    `handle_update_filter` at the correct position in the chain. The
    current order is:

        empty → ':' (content search) → '*' or '?' (glob/walk)
            → # / :tag: / :t: (tag filter) → >/< / .. (size filter)
            → fuzzy match

    Insert your guard **before** the more permissive parsers:
    - If your syntax uses a prefix character that could also appear in a
      filename (e.g. `date:`), insert it _before_ the fuzzy matcher but
      _after_ any parser that uses a conflicting prefix.
    - If your syntax uses a prefix character that cannot appear in a
      filename (e.g. `@`), you may insert it earlier, but still after
      the `:` content-search guard, because users type `:` to switch
      into content search and any preceding parser that matches a bare
      `:` breaks it.
    - Never insert your parser _after_ the fuzzy matcher. The fuzzy
      matcher returns `true` for almost everything, so anything after it
      is unreachable.

4. **`src/utils/search/doc.rs`** - add the new syntax to
    `render_search_markdown`. The doc is regenerated into
    `docs/search.md` by `cargo run --bin gen-docs`, and
    `crate::utils::search::tests::doc_is_in_sync` fails CI if the
    committed file drifts. If the syntax accepts units or a fixed set of
    operators (like `size.rs`'s `ACCEPTED_SIZE_UNITS`), expose the table
    as a `pub const` so the doc and the parser share one source of
    truth.

5. **`docs/search.md`** - **do not edit by hand**. Run
    `cargo run --bin gen-docs`. CI regenerates it.

6. **`po/pt_BR.po`** - any new user-facing message (e.g. "Invalid date
    filter") the parser surfaces through a toast.

## Parser shape

A parser that is well-behaved under the dispatch chain:

- is case-insensitive (query is already lowercased by the caller)
- strips the prefix it consumed and returns the remainder untouched so
  the next parser can run on it
- returns `None` for malformed input (e.g. `>10` without a unit), so
  the fuzzy matcher can treat the whole string as a filename - never
  returns `Some` with a partial parse
- never returns an error to the UI, if it cannot parse, it does not
  match and the next parser runs.

The size parser in `src/utils/search/size.rs` is the smallest complete
example. The tag parser in `tag.rs` shows how to expose a table
(`TAG_PREFIXES`) that both the parser and the doc renderer read.

## Where the reference code lives

- `src/ui/search/filter.rs` - the dispatch chain. Read every branch
  the wildcard/walk branch is the most subtle, because it kicks off a
  recursive walk (via `AppMsg::StartAdvancedSearch`) rather than
  filtering in place.
- `src/utils/search/size.rs` - the smallest full parser. Good template
  if the syntax has a prefix and a numeric payload.
- `src/utils/search/tag.rs` - the prefix-set parser. Good template if
  the syntax accepts one of several prefixes.
- `src/utils/search/content_query.rs` - the multi-segment parser
  (`:ext:term`). Good template if the query has a second `:` in it.
- `src/utils/search/doc.rs` - the doc renderer. Every `pub const` table
  referenced by a generated table in `docs/search.md` is defined here.

## Behaviour rules

- **Case-insensitivity is the caller's job.** Do not call
  `to_lowercase()` inside a parser. The caller passes a lowercased
  query, assume it.
- **Return the remainder, never re-parse it.** If your syntax is
  `foo:bar baz`, return the payload `bar` and the remainder `baz`.
  Do not attempt to parse `baz` inside your parser - the dispatch chain
  will run it through the next parser in order.
- **Do not touch `self.filter` before returning.** `handle_update_filter`
  assigns `self.filter = query.clone()` at the branch that handles your
  syntax. Doing it inside the parser would race with the outer branch.
- **Empty remainder is fine.** `>10MB` alone should return
  `Some(op, "")`, and `handle_update_filter`'s `parse_size_filter`
  branch handles the empty remainder (see the existing code - it filters
  the current view against the size alone).
- **A query that starts with your prefix but does not parse must return
  `None`.** `>10` (no unit) is not a size filter, it is a filename that
  happens to start with `>`. If your parser returns `Some` for a partial
  match, the fuzzy matcher never gets a chance and the file literally
  named `>10` becomes unreachable.
- **The `:` prefix is reserved.** `parse_content_search_query` claims
  anything that starts with `:` and does not start with `:tag:` or
  `:t:`. Do not add a syntax that starts with `:` without extending the
  content-query guard.
- **The `#` prefix is reserved.** `parse_tag_filter` claims `#`.
  Do not add a syntax that starts with `#`.

## Verifying the result

After wiring up the filter, it should:

- trigger for its prefix (`cargo test --lib utils::search::` covers
  the parser)
- not trigger for a filename that happens to start with the same
  characters but does not parse (`>10` must fall through to fuzzy)
- preserve the remainder for the next parser (`@foo bar` with an `@foo`
  filter should leave `bar` to the fuzzy matcher)
- appear in `docs/search.md` after `cargo run --bin gen-docs`
- pass `cargo test --lib utils::search::tests::doc_is_in_sync`
- not break `:term` content search, `#tag` filtering, `>size` filtering,
  or wildcard walks - run each of those queries manually before
  committing.
