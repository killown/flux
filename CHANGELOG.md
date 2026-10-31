# Changelog

All notable changes to Flux are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.7.0] - 2026-10-31

# Changelog

## [0.7.0] - 2026-10-31

### Added
- **Search**: Add "Save as tag" to label search results with a tag from the Search panel header.
- **Grid**: Drag the whole selection when dragging a selected item.
- **Grid**: Rubber-band select from an item with Ctrl+Shift.
- **Archive**: Offer to extract nested archives before entering them.
- **Properties**: Replace the Properties window with a right sidebar panel.
- **HWGA**: Add `hit!` probes to I/O, IPC, keyring, trash, archive and startup paths.

### Changed
- **Refactor**: Compile the application once, as the `flux` library.
- **Refactor**: Finish removing upward imports from `utils` and add a layering test.
- **Refactor**: Move `SENDER` to `infra` so `infra` no longer imports `app`.
- **Refactor**: Move the search launchers from `infra::search` to `app::update::search`.
- **Refactor**: Move `NEXT_TASK_ID` to `infra::tasks`.
- **Refactor**: Move loader `FluxApp` handlers to `app::update::loader`.
- **Refactor**: Invert `infra→app` dependency in thumbnails and trash.
- **Refactor**: Move toolkit-owning state to `app::state`, make `domain` GTK-free.
- **Refactor**: Group `FluxApp` session state into typed sub-structs.
- **Refactor**: Extract right-panel state into `Panels`.
- **Refactor**: Split `AppMsg` into per-topic sub-enums under `msg/`.
- **Refactor**: Extract header leaf widgets and panel revealers from `view!`.
- **Refactor**: Split `bind.rs` into focused submodules.
- **Refactor**: Split monolithic `preview.rs` into `preview/`.
- **Refactor**: Extract preview handlers, compute and probes into submodules.
- **Trash**: Delete top-level entry directly, stop recursing into trashed dirs.
- **UI**: Simplify `flux-card` border and hover styling.
- **Themes**: Make selection-status bar transparent.
- **Settings**: Open preferences as a non-modal window.

### Fixed
- **Search**: Stop pruning subfolders when the index wasn't queried.
- **Search**: Keep requesting an index delta scan on wildcard queries.
- **Search**: Drop UI-thread index query, reuse fuzzy matcher, fix `/run/user` check.
- **Search**: End the search layout stash on every reset.
- **Archive**: Fail zip writes when a directory entry cannot be read.
- **Archive**: Serialize zip rewrites with a per-archive lock.
- **Archive**: Refuse to overwrite existing zip entries on write.
- **Archive**: Refuse writes that would silently drop an existing folder's contents.
- **Archive**: Only purge scratch dirs matching `flux-<pid>-<random>` exactly.
- **Archive**: Harden extraction paths and cap entry sizes.
- **Undo**: Trash copies on undo instead of deleting permanently.
- **Undo**: Record rename paths without resolving the item symlink.
- **Undo**: Refuse move undo/redo when target path is occupied.
- **Paste**: Keep xattrs when a cut falls back to copy and delete.
- **Config**: Serialize saves and detect invalidation during load.
- **Build**: Install assets without stale binary, harden rerun and docs paths.
- **Cache**: Evict `folder_cache` by canonical key.
- **Rename**: Correct error toasts, detect dangling symlinks, evict cache.
- **Trash**: Empty trash on a worker thread and report failures.
- **Trash**: Allow deleting symlinks that point at protected folders.
- **Settings**: Apply the single-click setting to open tabs.
- **Inputs**: Make Ctrl/Shift+click not open files in single-click mode.
- **Inputs**: Attach background-click deselect per tab grid.
- **Recents**: Populate the active tab's grid instead of a detached one.
- **Recents**: Remove whole xbel bookmark blocks and write atomically.
- **Recents**: Rewrite `recently-used.xbel` off the GTK thread.
- **DnD**: Move symlinks as links instead of their targets.
- **DnD**: Stop drag-and-drop moves from overwriting existing files.
- **DnD**: Route drag-move through paste batch with conflict handling.
- **DnD**: Start drag when a single item is selected.
- **Quick-List**: Stop quick-transfer copy from overwriting existing files.
- **Clipboard**: Canonicalize copied paths off the GTK thread.
- **Preview**: Sniff `.ts`, decode UTF-16/Latin-1, highlight extensionless files.
- **Loader**: Show loading spinner on cache-hit folder loads.
- **Files**: Handle non-UTF-8 paths in CLI args and `--cwd` IPC.
- **UI**: Stop grid reflow when selecting a card.
- **UI**: Keep keyboard focus out of the header bar.
- **Sidebar**: Stop section headers from taking keyboard focus.
- **Sidebar**: Delay DnD drop-zone activation and shrink pin zone.
- **Shortcuts**: Make grid/list toggle work from any focus.
- **Grid**: Suspend rubberband selection while dragging an item.
- **LUKS**: Unescape octal sequences in mount points from `/proc/mounts`.
- **HWGA**: Skip exit report for crates with too few probes.

### Performance
- **Startup**: Split GTK init and add hwga probes to startup hot paths.
- **Startup**: Warm font and volume monitor early, drop duplicate sidebar build.
- **Startup**: Defer `StyleManager` init to avoid blocking startup.
- **Config**: Cache `Config` in an `Arc`, add `config_snapshot()`.
- **Network**: List network folders off the GTK thread.
- **Icon**: Preload icon theme files on a worker thread.
- **Search**: Speed up delta scan with a covering index and off-thread root collection.

### Documentation
- **Readme**: Add startup time and icon theme troubleshooting.
- **Testing**: Enforce layering rule via source scan.
- **Fuzz**: Fix target imports and add coverage for new modules.
- **Args**: Drop tests for removed `--file-properties` flag.

### Removed
- Remove compatibility re-exports left by the domain split.
- Remove unused `ui::search` compatibility shim.
- Remove unused `ui::config_handlers` compatibility shim.
- Remove unused `ui::navigation` compatibility shim.
- Remove unused `crate::model` re-export shim.
- Drop unused `SettingsWindow` re-export.
- Drop archive-drawn-as-folder icon swap.


## [0.6.0] - 2026-10-26

# Flux 0.6.0

Released 2026-10-26.

This release is the largest since Flux first shipped. The version brings a
full Quick List system, an embedded terminal, a new archive writer, a new
keymap, a rebuilt settings window, and a module reorganisation that moves
`model` and `services` into `domain` and `infra`. See
[Features](docs/features.md) for a description of the product as it stands
today.

## Highlights

- **Named Quick Lists.** The right-side Quick List panel holds any number of
  named lists of folders, each with its own slot. Lists persist across
  sessions, can be renamed, reordered, cleared and deleted, and are cycled
  with `Tab`, `Ctrl+PageUp/PageDown` and `Ctrl+1…9`.
- **Embedded terminal.** A full VT emulator with bottom-pane, right-panel
  and full-window positions, selected with `[ui.terminal].position`. The
  shell follows the grid directory through OSC 7, and the terminal serves
  its selection over a Unix socket so `flux-fm --selection` works from
  inside it.
- **Password-protected archives.** Zip, 7z and rar archives can be unlocked
  for the session. Passwords can be saved to the system keyring, and
  in-archive writes keep the archive's encryption scheme instead of
  silently stripping it.
- **Rebindable shortcuts.** Every shortcut that a user might reasonably
  want to change is now an entry in `[shortcuts]` in `config.toml`, editable
  from the Settings window with a click-to-capture picker. `"disabled"`
  unbinds a shortcut.
- **Configurable pane focus.** `Ctrl+Alt+Left`/`Ctrl+Alt+Right` move focus
  across the sidebar, the file grid and every open right-side panel.

## New

### Quick List

- Named, persistent Quick Lists, stored in SQLite and shown in a right-side
  panel. Each list has a slot, a delete button, a rename field and a
  collapse chevron.
- Quick List state is no longer wiped by opening a new window or by the
  sidebar refresh. A `--quick-list` given on the command line is appended
  as a new list instead of replacing the saved ones.
- `Insert` adds the current folder. `Ctrl+Shift+Insert` pins the selection
  to the sidebar. Tab and `Ctrl+PageUp/PageDown` cycle the selected list.
  `Ctrl+1…9` jump to a slot.
- "Send to Quick List" shows readable archive labels.

### Terminal

- New full-window terminal mode alongside the bottom pane and the
  right-side panel, selected with `[ui.terminal].position`. `F4` toggles
  whichever is configured.
- Full VT emulation: SGR, 256-colour and true-colour, alternate screen,
  bracketed paste, application cursor keys, DECSCUSR cursor style, ECH and
  REP.
- The selection is served over `$XDG_RUNTIME_DIR/flux/selection-<pid>.sock`
  and consumed by `flux-fm --selection`.
- Opt-in fish theme integration: when `ui.fish_theme` is on, Flux writes
  `~/.config/fish/conf.d/flux-theme.fish` from the active GTK theme.

### Archives

- Writing to password-protected zips now keeps the archive encrypted with
  the same cipher (ZipCrypto or WinZip AES) and the same password. Add,
  create-directory and remove all work.
- A padlock badge on the grid card shows encrypted archives, and the badge
  opens once the archive is unlocked for the session.
- Passwords are stored in the system keyring through `oo7`, keyed on the
  archive's canonical path and mtime. `builtin::forget_archive_password`
  removes them.
- TAR extraction preserves hard and soft links and refuses to write through
  symlinks.
- Extraction runs in an atomically created folder and counts nested files
  in the progress total. The progress poller stops if the extractor
  panics.

### Navigation

- `Ctrl+Alt+<letter>` cycles through folders whose name starts with that
  letter. `Ctrl+Alt+/` re-anchors the cycle, `Ctrl+Alt+\` cancels it.
- Navigate mode (`Ctrl+Insert`) turns plain letters into folder jumps and
  selects the first item of the entered folder.
- `--cwd PATH` navigates a running window, or starts a new one.
- `--toggle`, `--show` and `--hide` control a running window through a
  per-window Unix socket.

### Appearance and customization

- Theme engine with CSS themes under `~/.local/share/flux/themes/`,
  `~/.config/flux/themes/`, `/app/share/flux/themes/` and
  `/usr/share/flux/themes/`. `~/.config/flux/style.css` overrides every
  theme.
- The Appearance settings page is split into tabs (Layout, Opacity, Window,
  Theme, Icons), and all preference rows now use `adw::SwitchRow` and
  `adw::SpinRow`.
- Breadcrumb count and label length are configurable.
- Browsable archives can be drawn as folders in the grid
  (`ui.archives_as_folders`).
- Auto list mode: a folder with more items than `ui.auto_list_threshold`
  opens in list mode.
- Mute toggle in the preview header; video thumbnails and Ctrl+scroll zoom
  in the preview folder grid.
- Configurable header-button visibility and a `Ctrl+Shift+F` chord to
  toggle the filter bar.

### Settings and shortcuts

- Every rebindable shortcut has a click-to-capture picker and a
  Disable/Restore toggle. `shortcuts.* = "disabled"` unbinds it.
- The old `--menu-editor` and `--terminal` command-line flags are removed.
  The menu editor opens in-process from `F9`.
- `Ctrl+Shift+,` (and `Ctrl+<`) toggles list/grid mode.

## Fixed

- **Undo.** Rename ops keep their orientation on both stacks, so redo
  works. Trash restores the exact entry the deletion created, not every
  trash entry with the same original path. Copy and move record only the
  copies that created a destination, so undo never deletes a pre-existing
  file that was overwritten.
- **Paste.** A cut paste removes the source only after the write succeeds,
  and a failed or cancelled overwrite never deletes the destination.
  Tempfile prefixes are stripped only for files that live in Flux's own
  scratch directory.
- **Command execution.** Placeholders expand in a single left-to-right
  pass, so a filename containing `%f` cannot break the shell command. The
  editor path is single-quoted, with `'` inside it escaped.
- **Preview.** Syntax, Markdown and diff colours follow the light or dark
  scheme. Wide preview is bounded only in list mode, so the panel cannot
  grow the window past the monitor. Video unmutes after the mute toggle is
  turned off.
- **Search.** Exact extension matches and MIME patterns now work.
- **Selection.** The git diff panel does not auto-open over another open
  panel.
- **Sidebar.** Resizing stops at the rows' minimum width. Ctrl+right-click
  on a sidebar row opens the sidebar menu instead of the secondary menu.
- **Config.** The config directory is created before the first save. An
  on-disk state database that cannot be opened falls back to an in-memory
  one instead of aborting startup.
- **Icons.** Grid items rebind when a custom icon texture finishes
  decoding. LUKS loop-image mounts outside `/mnt` and `/media` are listed.
- **Terminal.** Cells stay on the grid after REP and ECH.
- **Shortcuts.** Panel-toggle chords are handled at capture phase, so they
  win over a focused widget inside the panel.
- **Build and packaging.** `msgfmt` failures fail the build.
  `make install DESTDIR=…` does not fetch remote themes.
  `set-archive-defaults` uses the installed desktop ID. The build script
  reruns when the branch ref or the sync/packaging environment changes.

## Performance

- File labels are pre-formatted off the GTK main thread, so list mode
  shows size and mtime without per-bind `localtime_r` calls.
- Cached folder validation, sorting and the watcher's download-sibling
  scan run off the GTK thread.
- Icon canonicalisation is skipped when no custom icons are configured.
  Custom file-icon decodes are deduplicated through the thumbnail
  semaphore.
- The ZIP lock probe runs off the folder-load path.
- Recursive network deletes run off the main thread.
- The release profile uses fat LTO and a single codegen unit.

## Internal

- `src/model` is now `src/domain`, and `src/services` is now `src/infra`.
  The `src/services` compatibility shim introduced during the
  reorganisation is removed. `ui/update` routers have moved into
  `app/update`.
- Modal dialogs migrated from `adw::Window` to `adw::Dialog`.
- `tests/` mirrors `src/` so a test target that is missing from the `mod`
  list is easier to spot.
- `gen-docs` drift detection now covers every file it writes, including
  `docs/cli.md`, `docs/config.md`, `docs/css-variables.md`,
  `docs/actions.md`, `docs/search.md`, `completions/flux-fm.fish` and
  `data/fish/flux-integration.fish`.
- New dev guides: `dev-architecture.md`, `dev-testing.md`, `dev-panel.md`,
  `dev-archive-backend.md`, `dev-menu-action.md`, `dev-search-filter.md`,
  `dev-keybinding.md`, `dev-message-bus.md` and `dev-config-option.md`.
- New `docs/features.md` overview page and `docs/demo.md` panel showcase.

## Breaking changes and migration

- **`ui.terminal_in_sidebar`** is replaced by **`ui.terminal.position`**
  (`"bottom"`, `"sidebar"`, `"fullscreen"`). `true` loads as `"sidebar"`,
  `false` or missing as `"bottom"`. The old key is dropped on the next save.
  See [Configuration ▸ Renamed and removed keys](docs/config.md#renamed-and-removed-keys).
- **`--menu-editor` and `--terminal`** are removed. `F9` opens the menu
  editor in-process; the terminal is opened with `F4`.
- **`Ctrl+Shift+,`** (and `Ctrl+<`) toggles list/grid mode.
- **`ui.max_content_search_results` default** is `0` (unlimited), matching
  the generated `docs/config.md`. A configured value still caps results.
- **Inline `[[sidebar]]` entries** remain the source of truth. Nothing
  needs migrating for users who never edited the file by hand.


## [0.5.0] - 2026-10-20

Flux 0.5.0 turns the preview panel into a real viewer: folders open as an
interactive grid, the panel can go full-screen, and video/audio can play in
sequence. It also removes two sources of main-thread stalls when folders
contain large custom icons.

### Added

- **Preview:** directories now open as an interactive grid instead of a flat
  listing. Navigate into folders and open files directly from the panel.
- **Preview:** full-screen mode, toggled by double-clicking the preview
  content. Double-click again or press `Esc` to leave.
- **Preview:** playlist mode for video and audio. When a track or clip ends,
  the panel advances to the next item automatically. Non-playable items are
  skipped, and playback stops at the end of the list.
- **Preview:** prev/next arrows are now shown only while the pointer is over
  the panel, so they no longer sit on top of content that isn't being
  interacted with.

### Changed

- Custom raster icons are decoded with a progressive downscale
  (`image::thumbnail`) instead of decoding at source resolution and scaling
  afterwards. Roughly 3–5x faster on multi-megapixel sources.

### Fixed

- Custom file icons are no longer decoded on the GTK main thread. Opening a
  folder with large custom icons no longer stalls the UI while each icon is
  decoded.
- Custom file icon decodes are throttled through the thumbnail semaphore, so
  a folder with hundreds of large icons can no longer starve thumbnail
  generation and file operations on the shared blocking pool.


## [0.3.0] - 2026-10-20

Flux 0.3.0 adds a `--preview` CLI flag and fixes two issues in the wide-preview mode introduced in 0.2.6.

### Features

- add `--preview` flag to open a folder with wide preview

### Bug Fixes

- open the `--preview` panel immediately and select the first item
- stop the wide-preview toggle from growing the window past the screen

### Chores

- reset the changelog, keep only 0.2.0+
- reorganize `pt_BR.po` by source file


## [0.2.5] - 2026-10-19

### Fixed

- Preview panel: window could grow beyond the screen width when opened
  in list mode. The grid's minimum width now no longer pushes the window,
  and the panel keeps a fixed minimum in wide mode instead of adding its
  natural width on top of the list.

### Added

- `Space` now toggles the preview panel _and_ the MIME filter together.
  `Ctrl+Space` and `Esc` close both. Explicit `Ctrl+M` filters survive
  closing the preview panel.
- Text and table previews are now clamped to a max content width so the
  floating prev/next arrows no longer overlap the content in wide mode.
- PDF previews open at 50% of the panel width by default (zoom still works).

### Translations

- Updated `pt_BR` for the new help strings.
