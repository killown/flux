# Search

Flux has four independent search surfaces, all sharing the same query language:

- **Live filter** - the search bar at the top of the window. Filters the current directory as you type. No disk walk.
- **Recursive search** - triggered by `*` or a `.ext` in the search bar, or by the **Search inside subfolders** switch in the advanced panel. Walks the current directory tree.
- **Content search** - triggered by `:term` or `:.ext:term` in the search bar. Reads file contents.
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

Examples:

```text
>10MB
<500KB
10MB..50MB
*.rs >10MB
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

Prefix the query with `:` to search inside file contents. The search walks the current directory recursively, reads each file in chunks, and returns the first matching line per file.

<!-- BEGIN GENERATED: content-syntaxes -->
| Syntax | Meaning |
|---|---|
| `:TERM` | Search all files for TERM |
| `:.EXT:TERM` | Search only files with extension EXT for TERM |
<!-- END GENERATED: content-syntaxes -->

The term must be at least 3 characters. Multiple extensions can be comma-separated:

```text
:hello
:.rs:hello
:.rs,py:hello
```

## Recursive walk

The search bar becomes a recursive walk when any of the following is true:

- The query contains `*` or `?`.
- The query contains `.` followed by at least one non-space character.
- The advanced panel's **Search inside subfolders** switch is on.
- The advanced panel's **Only search folders** switch is on.

Recursive results are shown in list mode, one row per file, with a relative path. The walk is batched into chunks of 500 results and flushed every 16 ms so the GTK main loop is never saturated.

## Advanced panel

The **Search** panel (right side, `Ctrl+f` or the toolbar magnifier) exposes the same query language through form fields. Fields map to the walker's parameters as follows:

| Field | Maps to |
|---|---|
| File name | Glob pattern(s), `,`-separated |
| Exact match | Glob pattern without wildcards |
| Regular expression | `regex:`-prefixed pattern |
| Inside files | Content search term |
| Pattern / Extension | Glob pattern(s) or MIME type(s), `,`-separated |
| Search inside subfolders | Recursive walk |
| Only search folders | Recursive walk with `is_dir` filter |
| Include hidden files | Overrides the session `show_hidden` flag |
| Modified | `>date:<unix>` injected into the query |
| File size | `size:<op><bytes>` injected into the query |

