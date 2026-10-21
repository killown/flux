//! Renderer for `docs/search.md`.
//!
//! The page interleaves hand-written prose (what the search bar does on each
//! keystroke, when recursive walk kicks in) with generated tables (accepted
//! units, accepted prefixes, accepted syntaxes). The renderer owns both, so
//! `doc_is_in_sync` in `search/mod.rs` catches any drift.
//!
//! Human editors should leave the `<!-- BEGIN GENERATED: … -->` regions
//! untouched, the prose around them is the part they can edit.

use super::content_query::CONTENT_QUERY_SYNTAXES;
use super::size::ACCEPTED_SIZE_UNITS;
use super::tag::TAG_PREFIXES;

/// Returns the complete `docs/search.md`, prose and generated tables.
#[allow(dead_code)] // used by `src/bin/gen-docs.rs`, not linked by the `flux-fm` binary alone
pub fn render_search_markdown() -> String {
    use std::fmt::Write as _;

    let mut out = String::new();

    out.push_str("# Search\n\n");
    out.push_str(
        "Flux has four independent search surfaces, all sharing the same \
         query language:\n\n\
         - **Live filter** - the search bar at the top of the window. \
         Filters the current directory as you type. No disk walk.\n\
         - **Recursive search** - triggered by `*` or a `.ext` in the search \
         bar, or by the **Search inside subfolders** switch in the advanced \
         panel. Walks the current directory tree.\n\
         - **Content search** - triggered by `:term` or `:.ext:term` in the \
         search bar. Reads file contents.\n\
         - **Tag filter** - triggered by `#name`, `:tag:name`, or `:t:name`. \
         Looks up files by extended-attribute tag.\n\n",
    );

    out.push_str("## Size filters\n\n");
    out.push_str(
        "Prepend `>` or `<` to a size token, or use a `..` range, to filter \
         by file size. A unit is mandatory - a bare number is rejected, \
         because `>10` matches everything and would lock the UI on large \
         trees.\n\n",
    );

    out.push_str("<!-- BEGIN GENERATED: size-units -->\n");
    out.push_str("| Unit | Byte multiplier |\n");
    out.push_str("|---|---|\n");
    for (name, mult) in ACCEPTED_SIZE_UNITS {
        let _ = writeln!(out, "| `{}` | {} |", name, mult);
    }
    out.push_str("<!-- END GENERATED: size-units -->\n\n");

    out.push_str(
        "The `ib` and `b` suffixes are stripped before the unit is looked up, \
         so `10MB`, `10MiB`, and `10M` all mean 10 MiB.\n\n\
         Examples:\n\n\
         ```text\n\
         >10MB\n\
         <500KB\n\
         10MB..50MB\n\
         *.rs >10MB\n\
         ```\n\n",
    );

    out.push_str("## Tag filters\n\n");
    out.push_str(
        "Prefix a comma-separated tag list with one of the following. All \
         three are equivalent.\n\n",
    );

    out.push_str("<!-- BEGIN GENERATED: tag-prefixes -->\n");
    out.push_str("| Prefix | Example |\n");
    out.push_str("|---|---|\n");
    for prefix in TAG_PREFIXES {
        let _ = writeln!(out, "| `{}` | `{}work,urgent` |", prefix, prefix);
    }
    out.push_str("<!-- END GENERATED: tag-prefixes -->\n\n");

    out.push_str(
        "Multiple tags are AND-ed. A free-text query after the tag list is \
         also AND-ed:\n\n\
         ```text\n\
         #work                  - files tagged \"work\"\n\
         #work,urgent           - files tagged both \"work\" and \"urgent\"\n\
         #work report           - files tagged \"work\" whose name contains \"report\"\n\
         ```\n\n",
    );

    out.push_str("## Content search\n\n");
    out.push_str(
        "Prefix the query with `:` to search inside file contents. The search \
         walks the current directory recursively, reads each file in chunks, \
         and returns the first matching line per file.\n\n",
    );

    out.push_str("<!-- BEGIN GENERATED: content-syntaxes -->\n");
    out.push_str("| Syntax | Meaning |\n");
    out.push_str("|---|---|\n");
    for (syntax, meaning) in CONTENT_QUERY_SYNTAXES {
        let _ = writeln!(out, "| `{}` | {} |", syntax, meaning);
    }
    out.push_str("<!-- END GENERATED: content-syntaxes -->\n\n");

    out.push_str(
        "The term must be at least 3 characters. Multiple extensions can be \
         comma-separated:\n\n\
         ```text\n\
         :hello\n\
         :.rs:hello\n\
         :.rs,py:hello\n\
         ```\n\n",
    );

    out.push_str("## Recursive walk\n\n");
    out.push_str(
        "The search bar becomes a recursive walk when any of the following is \
         true:\n\n\
         - The query contains `*` or `?`.\n\
         - The query contains `.` followed by at least one non-space \
         character.\n\
         - The advanced panel's **Search inside subfolders** switch is on.\n\
         - The advanced panel's **Only search folders** switch is on.\n\n\
         Recursive results are shown in list mode, one row per file, with a \
         relative path. The walk is batched into chunks of 500 results and \
         flushed every 16 ms so the GTK main loop is never saturated.\n\n",
    );

    out.push_str("## Advanced panel\n\n");
    out.push_str(
        "The **Search** panel (right side, `Ctrl+f` or the toolbar \
         magnifier) exposes the same query language through form fields. \
         Fields map to the walker's parameters as follows:\n\n\
         | Field | Maps to |\n\
         |---|---|\n\
         | File name | Glob pattern(s), `,`-separated |\n\
         | Exact match | Glob pattern without wildcards |\n\
         | Regular expression | `regex:`-prefixed pattern |\n\
         | Inside files | Content search term |\n\
         | Pattern / Extension | Glob pattern(s) or MIME type(s), `,`-separated |\n\
         | Search inside subfolders | Recursive walk |\n\
         | Only search folders | Recursive walk with `is_dir` filter |\n\
         | Include hidden files | Overrides the session `show_hidden` flag |\n\
         | Modified | `>date:<unix>` injected into the query |\n\
         | File size | `size:<op><bytes>` injected into the query |\n\n",
    );

    out
}
