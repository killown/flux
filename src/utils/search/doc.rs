//! Renderer for `docs/search.md`.
//!
//! The page interleaves hand-written prose (what the search bar does on each
//! keystroke, when recursive walk kicks in) with generated tables (accepted
//! units, accepted prefixes, accepted syntaxes). The renderer owns both, so
//! `doc_is_in_sync` in `search/mod.rs` catches any drift.
//!
//! Human editors should leave the `<!-- BEGIN GENERATED: … -->` regions
//! untouched, the prose around them is the part they can edit.

use super::content_query::{CONTENT_QUERY_SYNTAXES, MIN_CONTENT_TERM_LEN};
use super::size::ACCEPTED_SIZE_UNITS;
use super::tag::TAG_PREFIXES;
use crate::domain::model::constants::{SAVE_SEARCH_AS_TAG_MAX_FILES, SEARCH_SHORTCUT};
use crate::infra::constants::{
    CONTENT_SEARCH_BINARY_SNIFF_BYTES, CONTENT_SEARCH_CHUNK_BYTES, SEARCH_BATCH_SIZE,
    SEARCH_FLUSH_INTERVAL_MS,
};

/// Turns a GTK accelerator such as `<ctrl>f` into a label such as `Ctrl+F`.
fn accel_label(accel: &str) -> String {
    let mut out = String::new();
    let mut rest = accel;
    while let Some(tail) = rest.strip_prefix('<') {
        let Some((modifier, after)) = tail.split_once('>') else {
            break;
        };
        out.push_str(match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "primary" => "Ctrl+",
            "shift" => "Shift+",
            "alt" => "Alt+",
            "super" => "Super+",
            _ => "",
        });
        rest = after;
    }
    if rest.chars().count() == 1 {
        out.push_str(&rest.to_uppercase());
    } else {
        out.push_str(rest);
    }
    out
}

/// Returns the complete `docs/search.md`, prose and generated tables.
#[allow(dead_code)] // used by `src/bin/gen-docs.rs`, not linked by the `flux-fm` binary alone
pub fn render_search_markdown() -> String {
    use std::fmt::Write as _;

    let mut out = String::new();

    out.push_str("# Search\n\n");
    out.push_str(
        "Flux has four search surfaces:\n\n\
         - **Live filter** - the search bar at the top of the window. \
         A fuzzy, case-insensitive match on the file names of the current \
         directory, updated as you type. No disk walk.\n\
         - **Recursive search** - starts from the search bar when the query \
         contains `*` or `?` and is longer than one character, or from the \
         advanced panel. Walks the current directory tree.\n\
         - **Content search** - `:term` or `:.ext:term` in the search bar, \
         started with Enter, or the **Inside files** field of the advanced \
         panel. Reads file contents.\n\
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
         In the live filter, the rest of the query is a case-insensitive \
         substring match on the name, and folders always pass the size \
         filter. A query containing `*` or `?` is read as one glob, so it \
         cannot be combined with a size token.\n\n\
         Examples:\n\n\
         ```text\n\
         >10MB\n\
         <500KB\n\
         10MB..50MB\n\
         report >10MB\n\
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
        "Prefix the query with `:` to search inside file contents, then press \
         Enter. Content search never starts while you type. It walks the \
         current directory recursively and returns the first match of each \
         file.\n\n",
    );

    out.push_str("<!-- BEGIN GENERATED: content-syntaxes -->\n");
    out.push_str("| Syntax | Meaning |\n");
    out.push_str("|---|---|\n");
    for (syntax, meaning) in CONTENT_QUERY_SYNTAXES {
        let _ = writeln!(out, "| `{}` | {} |", syntax, meaning);
    }
    out.push_str("<!-- END GENERATED: content-syntaxes -->\n\n");

    let _ = write!(
        out,
        "The term must be at least {min} bytes long, not {min} characters. \
         Multiple extensions can be comma-separated:\n\n\
         ```text\n\
         :hello\n\
         :.rs:hello\n\
         :.rs,py:hello\n\
         ```\n\n\
         Details:\n\n\
         - Matching ignores case for ASCII letters only.\n\
         - Files are read in blocks of {chunk} KiB.\n\
         - A file is skipped when a NUL byte appears in its first {sniff} bytes.\n\
         - A file larger than `ui.content_search_max_file_mb` is skipped. \
         `0` removes the limit.\n\
         - The walk honors `.gitignore`, `.ignore`, the git global and exclude \
         files and the ignore files of parent directories. It follows symlinks \
         and skips hidden files unless the session shows them.\n\n",
        min = MIN_CONTENT_TERM_LEN,
        chunk = CONTENT_SEARCH_CHUNK_BYTES / 1024,
        sniff = CONTENT_SEARCH_BINARY_SNIFF_BYTES,
    );

    out.push_str("## Recursive walk\n\n");
    let _ = write!(
        out,
        "The search bar starts a recursive walk when the query contains `*` or \
         `?` and is longer than one character. The whole query is then one glob \
         pattern. A bare `.ext` does not start a walk, it is a fuzzy name \
         filter.\n\n\
         The advanced panel starts a walk when **Search inside subfolders**, \
         **Only search folders** or **Regular expression** is on, or when \
         **File name** contains `*` and **Exact match** is off.\n\n\
         The walk matches files only, never directories, unless **Only search \
         folders** is on. It honors the same ignore files and symlink rules as \
         content search, and it stops at `ui.max_search_results` results.\n\n\
         Results are shown in list mode, one row per file, with a path relative \
         to the current directory. They reach the window in batches: a batch is \
         flushed when it holds {batch} results or after {flush} ms, whichever \
         comes first, so the GTK main loop is never saturated. The batching \
         applies to this walk, not to content search.\n\n",
        batch = SEARCH_BATCH_SIZE,
        flush = SEARCH_FLUSH_INTERVAL_MS,
    );

    out.push_str("## Advanced panel\n\n");
    let _ = write!(
        out,
        "`{key}` (`shortcuts.search`) toggles the **Search** panel on the right \
         and switches the header to the search bar. The panel also opens from \
         the **Search** entry in the sidebar, a place with the path `search://` \
         that the first-run config adds.\n\n\
         | Field | Behavior |\n\
         |---|---|\n\
         | File name | `,`-separated terms. A plain term becomes `*term*`, and words separated by spaces become `*a*b*`. A term that starts or ends with `*` is used as typed. A `*` makes the search recursive unless Exact match or Regular expression is on. |\n\
         | Exact match | The term, lowercased, with no wildcards added. |\n\
         | Regular expression | A case-insensitive regex matched against the file name. Makes the search recursive. An invalid expression shows an error and nothing runs. |\n\
         | Inside files | Content search term, at least {min} bytes. When it is filled, File name, Exact match, Regular expression, Search inside subfolders, Only search folders and Include hidden files are ignored. Pattern / Extension is passed as the extension filter of the content search, so it takes bare extensions such as `rs, py`. |\n\
         | Pattern / Extension | `,`-separated. A bare word such as `rs` becomes `*.rs`, and an entry containing `/` is a MIME type expanded to its extension globs. When File name is also filled, the two are combined into one pattern. |\n\
         | Search inside subfolders | On by default. When off, the search only sets an extension filter on the current folder. |\n\
         | Only search folders | Makes the search recursive and matches directory names. |\n\
         | Include hidden files | Adds hidden files to the walk. It cannot hide files that the session's hidden-files toggle shows. Content search ignores this switch and follows the session toggle. |\n\
         | Modified | Any time, last hour, last 24 hours (`Today`), last 7, 30 or 365 days. Applied by the recursive walk. |\n\
         | File size | Larger or smaller than a number of KB, MB or GB, in units of 1024. The comparison is strict. Applied by the recursive walk. |\n\n",
        key = accel_label(SEARCH_SHORTCUT),
        min = MIN_CONTENT_TERM_LEN,
    );

    out.push_str("## Save as tag\n\n");
    let _ = write!(
        out,
        "The **Save as tag** button in the header of the **Search** panel labels \
         every file in the results on screen with one tag. A dialog asks for the \
         tag name, which is lowercased and loses its leading `#`, and has a **Pin \
         to sidebar** switch that adds the tag to the sidebar. The tag is added to \
         the tags a file already has.\n\n\
         The button is on only while there are between 1 and {max} results. It \
         works on the results of a recursive search, a content search, an \
         extension filter and a tag filter. It stays off while a live filter \
         hides part of the list, and in archives and other virtual locations.\n\n\
         The tag is a snapshot. Files that match the same search later are not \
         tagged automatically.\n\n",
        max = SAVE_SEARCH_AS_TAG_MAX_FILES,
    );

    out
}
