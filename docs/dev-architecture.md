# Architecture

A map of the codebase for contributors. Read it before the how-to guides: [Adding a right panel](dev-panel.md), [Adding an archive backend](dev-archive-backend.md), [Adding a menu action](dev-menu-action.md), [Adding a search filter](dev-search-filter.md) and [Adding a keybinding](dev-keybinding.md).

## Crates and entry points

`Cargo.toml` defines three targets:

| Target | Entry point | Purpose |
|---|---|---|
| library `flux` | `src/lib.rs` | Argument parsing, the doc generators, and the code that the integration tests link against. |
| binary `flux-fm` | `src/main.rs`, which calls `cli::run()` | The application. |
| binary `gen-docs` | `src/bin/gen-docs.rs` | Regenerates the derived documentation. |

`src/main.rs` declares its own `mod app`, `mod cli`, `mod domain`, `mod i18n`, `mod infra`, `mod ipc`, `mod model`, `mod services`, `mod shell_functions`, `mod ui` and `mod utils`. The same files are therefore compiled twice, once into the library and once into the binary. This has consequences:

- `cargo fmt --check` and `cargo clippy` report a problem once per crate.
- A crate-level lint such as `#![allow(clippy::result_large_err)]` must appear in both `lib.rs` and `main.rs`.
- A type that lives in `model` exists in both crates and the two are not interchangeable. `flux::args::WindowOptions` is the library's type, and `cli/launch.rs` copies it field by field into the binary's own.

## Startup

`cli::run` resolves the command line with `flux::args::resolve_startup_action` and then picks one of these paths:

1. Commands that need no display run first and exit: `--selection`, `--toggle`, `--show` and `--hide`. They are cheap enough to be called from every `$F` expansion in a shell.
2. The IPC server for `--cwd` starts before any client can connect. If another window already owns the socket, `--cwd` acts as a client and exits.
3. One-shot commands such as `--theme`, `--set-icon` and `--reset-icon` change `config.toml` and exit without a window.
4. Everything else reaches `cli/launch.rs`, which builds an `AppInit` and runs the Relm4 component `FluxApp`.

## Message flow

The UI follows Relm4's model, view, update pattern.

- `FluxApp` in `src/app/state/flux_app.rs` is the model. It owns the state and the widget references.
- `AppMsg` in `src/domain/model/msg.rs` is every event the app can receive. Each variant is doc-commented. `src/domain/` holds only types and logic: it never imports `ui`, `infra`, `app` or a GTK crate, so message payloads are plain data (URIs, `Image` pixels, `CancelHandle`).
- `FluxApp::handle_update` in `src/app/update/mod.rs` is the single entry point for messages. It passes the message through a chain of routers in this order: `navigation`, `loading`, `view`, `thumbnails`, `search`, `file_ops`, `dialogs`, `context_menu`, `network`, `git`, `window` and `sidebar`.
- Each router exposes `handle(app, message, sender) -> Result<(), AppMsg>`. `Ok(())` means it consumed the message. `Err(message)` hands the message to the next router.
- A message that no router claims is logged as `[flux] unhandled AppMsg`. A new `AppMsg` variant that you never routed shows up there, not as a compile error.

Routers stay thin. The real work lives in `handle_*` methods on `FluxApp` under `src/app/update/` (grouped by area) and `src/ui/dialog/`.

## Threads

GTK objects belong to the main thread. Everything that can block runs elsewhere and reports back with an `AppMsg`.

- **Background threads** reach the app through `infra::SENDER`, a global `relm4::Sender<AppMsg>`. The `--cwd` server, the `goto` watcher and the selection server all use it.
- **Questions that need the UI** are forwarded as a message with a reply channel. The selection server sends `AppMsg::QuerySelection { reply }` and waits up to two seconds for the main thread to answer.
- **Directory loading** runs on a dedicated Rayon pool in `src/infra/loader/pool.rs`. It has between 4 and 8 threads with 2 MiB stacks, so the first large folder does not commit the global pool's much larger stacks. Results reach the grid in batches of `ui.loader_batch_size` items.
- **Subprocesses, network work and transfers** use Tokio, and short blocking jobs use `relm4::spawn_blocking`. A dispatch arm or message handler on the main thread should return in microseconds.

## Persistence

- **`config.toml`** is read through `utils::load_config`, which keeps a cached copy. `save_config` primes that cache and `invalidate_config_cache` drops it. A config watcher in `src/cli/watcher.rs` reloads the file while Flux runs. Migrations of renamed keys run when the file is loaded, for example `ui.terminal_in_sidebar`, and are listed in [Configuration](config.md).
- **SQLite state** lives behind `StateManager` in `src/infra/db/`. It runs in WAL mode and holds per-folder view settings, location history and file tags, with one module per area: `views`, `history`, `tags`, `icons` and `maintenance`.
- **Shell bridge files** under `$XDG_CACHE_HOME/flux/` are described in [Shell integration](shell-integration.md).

## Source map

| Path | Contents |
|---|---|
| `src/args.rs`, `src/cli/` | Argument parsing and the non-GUI commands. |
| `src/ipc.rs` | The `--cwd` Unix socket server and client. |
| `src/shell_functions.rs` | The fish helper source, shared by the terminal snippet and `gen-docs`. |
| `src/domain/model/` | `FluxApp`, `AppMsg`, shared types, `config/` structs and their defaults. `src/model/` only re-exports it. |
| `src/app/update/` | The message routers (`mod.rs` plus one router per area) and the `handle_*` methods that do the work, grouped by area: `file_ops/` (clipboard, paste, rename, trash, links, commands), `navigation/`, `view/`, `search/`, `settings/`, `git/`, `sidebar/`, `window/`, `helpers/`, plus `tag.rs`, `task.rs`, `undo.rs`, `terminal.rs` and `watcher.rs`. |
| `src/ui/panels/` | Right-side panels: tag, search, diff, location, connect, preview (`preview/`, which also holds the `syntax`, `markdown`, `table`, `gutter`, `binary`, `office`, `folder_thumbs` and `registry` helpers), actions, terminal and properties (`properties/`). |
| `src/ui/dialog/` | Dialogs. |
| `src/ui/menu/` | The context menu, the Ctrl+right-click menu and the menu editor. |
| `src/ui/inputs/`, `src/ui/keymap/` | Keyboard handling and the configurable shortcut table. |
| `src/ui/components/` | The file card and list row, its builder and its `bind`. |
| `src/ui/settings/`, `src/ui/help/` | The settings window and the help window. |
| `src/ui/startup/` | Building the window, sidebar, tabs and grid. |
| `src/ui/debug/` | The debug window. |
| `src/infra/loader/` | Directory, archive, network and Recents loading. |
| `src/infra/archive/` | The `ArchiveBackend` trait and one backend per format. |
| `src/infra/search/` | The file indexer, per-file content scanning and the forbidden-path filter. The recursive filename and content walks are in `src/app/update/search/`. |
| `src/infra/thumbnails/`, `src/infra/tasks/` | Thumbnail generation and the file-operation task queue. |
| `src/infra/terminal/` | The embedded terminal: PTY, emulator, drawing, and the shell bridge. |
| `src/infra/` (others) | `db`, `git`, `inspector`, `luks`, `mounts`, `network`, `trash`, `fonts.rs`, `window_control.rs`. `src/services/` only re-exports it. |
| `src/utils/` | Config loading, path and glob helpers, media and icon helpers, search parsers and `hwga`. |

## Debug and performance tooling

- `hit!()` adds a call counter to a function. `hit!("name")` gives it a stable name. In release builds both compile to nothing. The `hwga` module in `src/utils/hwga/` collects the counts and flags calls on the main thread that take longer than 16 ms, one 60 Hz frame.
- Tests can read the counters. `tests/hwga_regression_test.rs` uses `hwga::count` to assert that icon lookups do not rescan directories again and again. Such tests only exist in debug builds.
- Debug builds time every call to `handle_update` and report slow messages.
- `Ctrl+Shift+F7`, bound to `shortcuts.open_debug_window`, opens a debug window with process and memory diagnostics.

## Build script

`build.rs` compiles the GResource bundle from `data/resources.gresource.xml` and exports the short git hash as `FLUX_GIT_HASH`. When you build with the `local` profile and set `FLUX_SYNC_ASSETS`, it also runs `cargo fmt` and `make install-assets`, which installs everything except the binary because build scripts run before the crate is compiled; run `make install` afterwards to update the binary. It skips both when `SOURCE_DATE_EPOCH` is set, which is how packaging tools such as Arch's `makepkg` build.

## Generated files

`cargo run --bin gen-docs` writes every file below. A test in the generating module fails when the committed copy is stale. See [Testing and CI](dev-testing.md).

| Output | Generated from |
|---|---|
| `docs/cli.md` | `src/doc_cli.rs` |
| `docs/config.md` | `src/domain/model/doc_config.rs` and the config structs |
| `docs/css-variables.md` | `src/ui/doc_css_variables.rs` and `src/ui/style.css` |
| `docs/actions.md` | The `BUILTINS` table in `src/ui/menu/editor/builtins.rs` |
| `docs/search.md` | `src/utils/search/doc.rs` |
| `completions/flux-fm.fish` | The `CLI_OPTIONS` table in `src/args.rs`, through `src/doc_fish.rs` |
| `data/fish/flux-integration.fish` | `src/shell_functions.rs`, through `src/doc_fish.rs` |
