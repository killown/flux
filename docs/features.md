# Features

Flux is a fast, keyboard-driven file manager built with GTK4 and Libadwaita.
This page lists what it does, grouped by area. Each section links to the
reference page that documents the details.

- [Navigation](#navigation)
- [File operations](#file-operations)
- [Archives](#archives)
- [Network locations](#network-locations)
- [Search](#search)
- [Preview](#preview)
- [Panels](#panels)
- [Terminal](#terminal)
- [Sidebar](#sidebar)
- [Context menus and actions](#context-menus-and-actions)
- [Customization](#customization)
- [Keyboard](#keyboard)
- [Command line](#command-line)
- [Diagnostics](#diagnostics)

## Navigation

- **Tabs.** Each tab keeps its own path, history, sort order, filter, list
  or grid mode, and scroll position. Middle-click a folder to open it in a
  new tab, Ctrl+middle-click for a new window. Drag a tab to reorder it.
- **History.** Back and forward move through a per-tab history stack.
  `Ctrl+Back` on a mouse jumps to the most recent location.
- **Breadcrumbs.** The header bar shows the last `ui.breadcrumb_count` path
  segments. Double-click the breadcrumb bar for a path entry. Long paths
  are truncated to `ui.breadcrumb_max_chars` per segment.
- **Location panel.** `Ctrl+L` opens a path entry with the location
  history. Arrow-down jumps into the list, Enter navigates.
- **Quick List.** A named list of folders on the right, cycled with Tab and
  `Ctrl+PageUp/PageDown`, jumped to with `Ctrl+1…9`. See
  [Panels ▸ Quick List](#quick-list).
- **Navigate mode.** `Ctrl+Insert` turns letters into folder jumps: type
  `s` to enter the first folder starting with `s`, repeat to cycle.
  Enter rebases the mode inside the folder, Backspace walks back.
- **Recent locations.** `recent:///` lists the GTK recent-files registry,
  filterable, sortable, and clearable.
- **Trash.** `trash:///` lists the system trash. Entries can be restored
  from the context menu. The Trash row in the sidebar is removable.

## File operations

- **Copy, cut, paste.** Standard `Ctrl+C`/`Ctrl+X`/`Ctrl+V`, including
  cross-instance paste through `text/uri-list`. Files pasted from inside
  an archive are extracted first.
- **Drag and drop.** Files and folders can be dragged between grid cells,
  onto sidebar rows, onto Quick List rows, and between Flux windows. Drag
  and drop can be disabled from Settings.
- **Conflict resolution.** When a destination exists, a dialog offers
  **Replace**, **Skip**, **Auto-Rename** and **Apply to all**. Auto-Rename
  produces `file (1).txt`, `file (2).txt`, and so on.
- **Rename.** `F2` edits the filename inline. Extension and path
  separators are handled: renaming `a.tar.gz` to `b` keeps `.tar.gz` out
  of the selection.
- **Trash and restore.** `Delete` sends the selection to the system trash.
  `Ctrl+Z` restores exactly the entry the deletion created, so
  delete-recreate-delete sequences undo in the right order.
- **Undo and redo.** `Ctrl+Z` and `Ctrl+Shift+Z`/`Ctrl+Y` cover renames,
  moves, copies and trashes for the current session.
- **Links.** `Ctrl+Shift+V` creates symlinks from clipboard paths,
  `Ctrl+Alt+Shift+V` hardlinks. Conflicts are auto-renamed.
- **Drop to target.** Middle-click on a Quick List row removes the folder
  from the list. Drag files onto it to move them.
- **Batch creation.** The New Folder and New File dialogs accept a
  comma-separated list and create every entry, reporting a summary.
- **Progress.** File transfers over a size or count threshold open a
  progress dialog with per-task tabs, transfer speed and ETA. Small
  transfers show a progress bar in the status bar.

## Archives

- **Browse without extracting.** Zip, 7z, tar (all common compressions),
  rar, iso, deb, and single-file gz/bz2/xz/zst/lz4 are browsable as
  virtual directories through the `archive://` URI scheme.
- **Password-protected archives.** A password prompt unlocks the archive
  for the session. Passwords can be saved to the system keyring. Zip, 7z
  and rar are supported.
- **Writing inside archives.** Files can be dropped or pasted into a zip,
  directories can be created, and entries can be deleted, including in
  password-protected zips. The archive keeps its encryption scheme.
- **Thumbnails from archives.** Image and video previews are generated
  from inside the archive without extracting the whole file.
- **In-archive thumbnail cache.** For zip archives, generated thumbnails
  can be stored in a hidden `.flux-cache/` folder inside the zip itself,
  so the next visit is fast. Off by default.
- **Extract.** `Extract` in the header unpacks the current archive into a
  sibling `<name>_extracted` folder, with byte-level progress.
- **Archives as folders.** Optionally drawn like folders in the grid. The
  archive stays a file for drag and drop, delete and menu actions.

## Network locations

- **Protocols.** SMB, SFTP, FTP, FTPS, WebDAV, NFS, AFP, MTP, PTP,
  Google Drive, and the `network:///` neighbourhood, all through GVFS.
- **Connect panel.** `Ctrl+Shift+L` opens a form with protocol, host,
  port, share, and username. Passwords are handled by GVFS.
- **Mounts.** Active network mounts appear in the sidebar under
  **Connected Servers**. Eject unmounts them.
- **Credentials.** The dialog appears when a server asks for credentials,
  and again with an error hint on a wrong password.
- **LUKS.** Double-clicking a LUKS image unlocks it through `udisksctl`
  with a passphrase and mounts the resulting device.

## Search

Four search surfaces, all documented in [search](search.md):

- **Live filter.** A fuzzy, case-insensitive match on filenames of the
  current directory. No disk walk.
- **Recursive search.** Walks the tree when the query contains `*` or `?`,
  or from the advanced panel. Honors `.gitignore`, `.ignore` and the git
  ignore files. Batches results so the UI never stalls.
- **Content search.** `:term` or `:.ext:term` in the search bar, started
  with Enter. Reads file contents in fixed-size chunks with a NUL-byte
  binary sniff.
- **Tag filter.** `#name`, `:tag:name` or `:t:name` filter by extended
  attribute tags. Tags are stored in `user.xdg.tags`.

An optional SQLite FTS5 index makes filename search near-instantaneous.
The index is off by default and can be enabled in Settings.

## Preview

- **Peek panel.** `Ctrl+Space` opens a preview of the selection, or `Space`
  to also apply the item's MIME filter. The panel follows selection
  changes while open.
- **Type-specific viewers.** Image (up to 2560 px), PDF (rendered with
  Poppler, multi-page, zoom, invert), text (syntect-highlighted with a
  line-number gutter), Markdown (rendered, with a source toggle),
  CSV/TSV (virtualised table), folder listing (grid with lazy thumbnails),
  archive listing (same grid), audio (with mpv IPC controls), video
  (GStreamer), font specimen, executable metadata, and a hex dump for
  binary files.
- **Details pane.** Name, type, size, modified, permissions, symlink
  target, and every `FileInfo` extra the viewer produced.
- **Zoom.** Ctrl+scroll, the header controls, or the `100%` reset. Works
  on images, PDFs, text and Markdown.
- **Playlist mode.** With audio or video selected, play the next item when
  playback ends.
- **Full screen.** The preview fills the window; the grid and header are
  hidden. Esc leaves it.

## Panels

Every right-side panel is mutually exclusive with the others, so only one
is open at a time. Each one slides in from the right and can be resized.

- **Preview.** See [Preview](#preview).
- **Search.** The advanced search form: file name, exact match, regex,
  inside files, pattern/extension, scope, hidden files, modified date,
  file size.
- **Tags.** Lists every known tag with actions to apply, remove, pin to
  the sidebar, or delete globally.
- **Git diff.** Shows the working-tree diff of the selected file with
  colour-coded additions and deletions. Clicking a hunk opens the file at
  that line in `ui.diff_editor`.
- **Contextual actions.** MIME-driven actions from `menu-*.rs` templates,
  filtered with a search box. Includes an **Edit template** button and a
  **New Action** dialog.
- **Quick List.** Every named list, with a fuzzy filter, per-list
  collapse, reorder, rename, and quick transfer. See [Quick List](#quick-list).
- **Location.** Path entry and location history. See
  [Navigation ▸ Location panel](#navigation).
- **Connect to server.** GVFS connection form. See
  [Network locations](#network-locations).
- **Terminal.** A right-side terminal. See [Terminal](#terminal).

## Terminal

- **Bottom pane, right panel, or full window.** The position is set in
  Settings (`bottom`, `sidebar`, `fullscreen`). `F4` toggles whichever is
  configured.
- **Fish integration.** When `ui.fish_theme` is on, Flux writes
  `~/.config/fish/conf.d/flux-theme.fish` from the active GTK theme so the
  prompt and colours match the rest of the window. The file is inert
  outside Flux.
- **Shell selection.** `ui.terminal.shell` picks the shell. It defaults to
  `$SHELL`, then `/bin/bash`.
- **Full VT emulation.** Cursor movement, SGR, 256-colour and true-colour
  SGR, alternate screen, bracketed paste, application cursor keys, DECSCUSR
  cursor style, OSC 7 for working-directory sync, OSC 11 for background
  colour, and mouse selection with primary-clipboard copy.
- **Synchronized CWD.** Following a `cd` in the terminal navigates the
  file grid to the same directory, when neither side is inside an archive
  or on a network location.
- **Shell helpers.** `flux_goto`, `flux_cwd`, `flux_selection`, `$F`, and
  the `flux_prompt` integration are written to
  `~/.local/share/fish/flux-integration.fish` and sourced by the theme
  snippet.

## Sidebar

The left sidebar lists pinned locations, sections, and network mounts.
Right-click empty space to add sections, panel shortcuts, or manage
templates.

- **Sections.** `kind = "label"` entries render as non-interactive section
  headers. They can be created, renamed, removed and reordered.
- **Pinning.** Drag a folder from the grid onto the sidebar, or onto a
  specific row to insert it there. `Ctrl+Shift+Insert` pins the selection
  permanently.
- **Icons.** Right-click a row to change its icon. Only symbolic icons are
  offered, so the sidebar follows the theme.
- **Recents.** The **Recents** row is optional and its position is
  configurable through `ui.recents_row`.
- **XDG directories.** `ui.show_xdg_dirs` shows the standard XDG folders
  above the custom entries.
- **Network mounts.** Active GVFS mounts appear under **Connected
  Servers**, with an eject button.

## Context menus and actions

The right-click menu is defined by `~/.config/flux/menu.rs`. Every line
follows the DSL documented in [actions](actions.md):

```text
"Label" => "mime_types", "command"[, "toast"][, "no_command_dialog"]
```

- **Shell commands.** `%p`, `%d`, `%f` and `%l` are expanded in a single
  pass and shell-quoted, so filenames cannot break the command.
- **Builtin actions.** `builtin::*` actions are handled by Flux directly:
  copy, cut, paste, rename, delete, tag, icon pickers, background setters,
  and so on. See [actions](actions.md#builtin-actions).
- **Submenus.** A `Submenu > Label` prefix nests the entry.
- **Contextual actions panel.** Ctrl+right-click, or the panel itself,
  reads a `menu-<category>-<subtype>.rs` template chosen by the item's
  MIME type, falling back to `menu-<category>-all.rs` and `menu-all.rs`.
- **Menu editor.** `F9` opens a full editor for `menu.rs` and any
  `menu-*.rs` file, with line numbers, drag reordering, and a builtin
  picker.

## Customization

- **Config.** `~/.config/flux/config.toml`. Every key is documented in
  [config](config.md). Defaults are chosen to be sensible; the file is
  rewritten whenever the user changes something in Settings.
- **Themes.** A custom CSS file per theme, in
  `~/.config/flux/themes/`, `~/.local/share/flux/themes/`,
  `/app/share/flux/themes/`, or `/usr/share/flux/themes/`. Light, dark and
  system themes are honoured through `adw::StyleManager`.
- **User overrides.** `~/.config/flux/style.css` loads on top of the theme
  and always wins. The same file is documented in
  [css-variables](css-variables.md), where every custom property and every
  GTK named colour has a table row with a use count.
- **Background images.** Set per slot (window, left sidebar, right
  panel). The tint opacity is adjustable per slot.
- **Generated extension icons.** When the theme has no icon for an
  extension, Flux synthesises one from a user-editable SVG template
  (`~/.local/share/flux/icons/template.svg`) and caches it under
  `~/.local/share/flux/icons/extensions/generated/`.
- **Custom icons.** Files and folders can be given a custom image, or a
  theme icon by name. Extension-wide custom icons are stored under
  `~/.local/share/flux/icons/extensions/custom/`.
- **Fonts.** Font files can be installed into `~/.local/share/fonts/`
  from the grid, or from inside an archive.
- **UI scale.** `ui.ui_scale` scales the whole interface without changing
  the display DPI.
- **Shortcuts.** Every chord is rebindable from Settings. `"disabled"`
  unbinds a shortcut; the trigger is not registered and the chord falls
  through to the focused widget.

## Keyboard

The full shortcut reference is in the **Help** window (`F1`), and every
chord is editable from Settings. Highlights:

|                               |                                |
| ----------------------------- | ------------------------------ |
| `Ctrl+F`                      | Focus search                   |
| `Ctrl+L`                      | Location panel                 |
| `Ctrl+H`                      | Toggle hidden files            |
| `Ctrl+M`                      | Filter by selected item's MIME |
| `Ctrl+Space`                  | Toggle preview                 |
| `Space`                       | Toggle preview and MIME filter |
| `Ctrl+T` / `Ctrl+W`           | New tab / close tab            |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab            |
| `F1`                          | Keyboard shortcuts             |
| `F2`                          | Rename                         |
| `F3`                          | Change folder icon             |
| `F4`                          | Toggle terminal                |
| `F5`                          | Refresh                        |
| `F6`                          | Toggle header bar              |
| `F7`                          | Toggle status bar              |
| `F8`                          | Toggle sidebar                 |
| `F9`                          | Menu editor                    |
| `F10`                         | Preferences                    |
| `F11`                         | Toggle wide preview            |
| `Ctrl+Z` / `Ctrl+Shift+Z`     | Undo / redo                    |
| `Ctrl+Shift+A`                | Contextual actions panel       |
| `Ctrl+Shift+L`                | Connect to server panel        |
| `Ctrl+Shift+T`                | Tag panel                      |
| `Ctrl+Shift+Q`                | Quick List panel               |
| `Ctrl+Insert`                 | Toggle Navigate mode           |

Type-to-search, `#tag` filters, `:term` content search, `>10MB` size
filters, and `*.glob` recursive patterns all start from the search bar or
the grid.

## Command line

The full reference is in [cli](cli.md). Summary:

- Open a path, a set of paths, or an archive directly.
- `--no-sidebar`, `--no-header`, `--no-statusbar` for a session-only
  minimal window.
- `--file-properties PATH` opens the properties window for a file.
- `--set-icon TARGET IMAGE` and `--reset-icon TARGET` manage custom icons.
- `--set-icons-stdin` reads `TARGET<TAB>IMAGE` pairs from standard input,
  so a script can set many icons in one process.
- `--cwd PATH` navigates the running window, or starts a new one.
- `--selection` and `--selection -0` print the selected paths, used by
  the `flux_selection` fish function.
- `--toggle`, `--show`, `--hide` show or hide a running window through a
  per-window Unix socket.

## Diagnostics

- **Debug window.** `Ctrl+Shift+F7` opens a memory profiler with RSS,
  VmPeak, VmData, VmSwap, open file descriptors, thread count, glibc
  `mallinfo2`, mimalloc statistics, `smaps` categories, top anonymous
  and mapped-file regions, folder-cache contents, font mappings, and the
  telemetry table. A **Purge Memory** button forces mimalloc and glibc to
  return free pages to the OS, and **Save report** writes the whole thing
  to a file.
- **Telemetry.** A lock-free probe network (`hwga`) records per-function
  call counts, total and self time, p50/p95/p99 buckets, main-thread
  over-budget counts, and peak in-flight. The table is printed at exit,
  shown in the debug window, and can be exported to JSON with
  `FLUX_HWGA_OUT=1`.
- **Debug logs.** `FLUX_DEBUG_NETWORK=1` traces GVFS listing.
  `FLUX_DEBUG_SEARCH=1` traces the search walk. Both default to silent.

## See also

- [config](config.md) - every key in `config.toml`, with defaults.
- [actions](actions.md) - the `menu.rs` DSL, placeholders, and builtin
  actions.
- [search](search.md) - every search surface, in detail.
- [css-variables](css-variables.md) - every variable and named colour the
  stylesheet reads.
- [cli](cli.md) - every command-line flag.
