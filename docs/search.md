# Search

Flux has four search surfaces:

- **Live filter** - the search bar at the top of the window. A fuzzy, case-insensitive match on the file names of the current directory, updated as you type. No disk walk.
- **Recursive search** - starts from the search bar when the query contains `*` or `?` and is longer than one character, or from the advanced panel. Walks the current directory tree.
- **Content search** - `:term` or `:.ext:term` in the search bar, started with Enter, or the **Inside files** field of the advanced panel. Reads file contents.
- **Tag filter** - triggered by `#name`, `:tag:name`, or `:t:name`. Looks up files by extended-attribute tag.

## Size filters

Prepend `>` or `<` to a size token, or use a `..` range, to filter by file size. A unit is mandatory - a bare number is rejected, because `>10` matches everything and would lock the UI on large trees.

<!-- BEGIN GENERATED: size-units -->
| Unit | Byte multiplier |
|---|---|
| `k` | 1024 |
| `m` | 1048576 |
| `g` | 1073741824 |
| `t` | 1099511627776 |
<!-- END GENERATED: size-units -->

The `ib` and `b` suffixes are stripped before the unit is looked up, so `10MB`, `10MiB`, and `10M` all mean 10 MiB.

In the live filter, the rest of the query is a case-insensitive substring match on the name, and folders always pass the size filter. A query containing `*` or `?` is read as one glob, so it cannot be combined with a size token.

Examples:

```text
>10MB
<500KB
10MB..50MB
report >10MB
```

## Tag filters

Prefix a comma-separated tag list with one of the following. All three are equivalent.

<!-- BEGIN GENERATED: tag-prefixes -->
| Prefix | Example |
|---|---|
| `:tag:` | `:tag:work,urgent` |
| `:t:` | `:t:work,urgent` |
| `#` | `#work,urgent` |
<!-- END GENERATED: tag-prefixes -->

Multiple tags are AND-ed. A free-text query after the tag list is also AND-ed:

```text
#work                  - files tagged "work"
#work,urgent           - files tagged both "work" and "urgent"
#work report           - files tagged "work" whose name contains "report"
```

## Content search

Prefix the query with `:` to search inside file contents, then press Enter. Content search never starts while you type. It walks the current directory recursively and returns the first match of each file.

<!-- BEGIN GENERATED: content-syntaxes -->
| Syntax | Meaning |
|---|---|
| `:TERM` | Search all files for TERM |
| `:.EXT:TERM` | Search only files with extension EXT for TERM |
<!-- END GENERATED: content-syntaxes -->

The term must be at least 3 bytes long, not 3 characters. Multiple extensions can be comma-separated:

```text
:hello
:.rs:hello
:.rs,py:hello
```

Details:

- Matching ignores case for ASCII letters only.
- Files are read in blocks of 256 KiB.
- A file is skipped when a NUL byte appears in its first 1024 bytes.
- A file larger than `ui.content_search_max_file_mb` is skipped. `0` removes the limit.
- The walk honors `.gitignore`, `.ignore`, the git global and exclude files and the ignore files of parent directories. It follows symlinks and skips hidden files unless the session shows them.

## Recursive walk

The search bar starts a recursive walk when the query contains `*` or `?` and is longer than one character. The whole query is then one glob pattern. A bare `.ext` does not start a walk, it is a fuzzy name filter.

The advanced panel starts a walk when **Search inside subfolders**, **Only search folders** or **Regular expression** is on, or when **File name** contains `*` and **Exact match** is off.

The walk matches files only, never directories, unless **Only search folders** is on. It honors the same ignore files and symlink rules as content search, and it stops at `ui.max_search_results` results.

Results are shown in list mode, one row per file, with a path relative to the current directory. They reach the window in batches: a batch is flushed when it holds 500 results or after 16 ms, whichever comes first, so the GTK main loop is never saturated. The batching applies to this walk, not to content search.

## Advanced panel

`Ctrl+F` (`shortcuts.search`) toggles the **Search** panel on the right and switches the header to the search bar. The panel also opens from the **Search** entry in the sidebar, a place with the path `search://` that the first-run config adds.

| Field | Behavior |
|---|---|
| File name | `,`-separated terms. A plain term becomes `*term*`, and words separated by spaces become `*a*b*`. A term that starts or ends with `*` is used as typed. A `*` makes the search recursive unless Exact match or Regular expression is on. |
| Exact match | The term, lowercased, with no wildcards added. |
| Regular expression | A case-insensitive regex matched against the file name. Makes the search recursive. An invalid expression shows an error and nothing runs. |
| Inside files | Content search term, at least 3 bytes. When it is filled, File name, Exact match, Regular expression, Search inside subfolders, Only search folders and Include hidden files are ignored. Pattern / Extension is passed as the extension filter of the content search, so it takes bare extensions such as `rs, py`. |
| Pattern / Extension | `,`-separated. A bare word such as `rs` becomes `*.rs`, and an entry containing `/` is a MIME type expanded to its extension globs. When File name is also filled, the two are combined into one pattern. |
| Search inside subfolders | On by default. When off, the search only sets an extension filter on the current folder. |
| Only search folders | Makes the search recursive and matches directory names. |
| Include hidden files | Adds hidden files to the walk. It cannot hide files that the session's hidden-files toggle shows. Content search ignores this switch and follows the session toggle. |
| Modified | Any time, last hour, last 24 hours (`Today`), last 7, 30 or 365 days. Applied by the recursive walk. |
| File size | Larger or smaller than a number of KB, MB or GB, in units of 1024. The comparison is strict. Applied by the recursive walk. |

## Save as tag

The **Save as tag** button in the header of the **Search** panel labels every file in the results on screen with one tag. A dialog asks for the tag name, which is lowercased and loses its leading `#`, and has a **Pin to sidebar** switch that adds the tag to the sidebar. The tag is added to the tags a file already has.

The button is on only while there are between 1 and 500 results. It works on the results of a recursive search, a content search, an extension filter and a tag filter. It stays off while a live filter hides part of the list, and in archives and other virtual locations.

The tag is a snapshot. Files that match the same search later are not tagged automatically.

