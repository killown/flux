# Testing and CI

`tools/tests.sh` is the gate every change must pass. It is also the fastest way to find out why CI is red.

## The gate

```bash
sh tools/tests.sh
```

The script stops at the first failing step (`set -e`):

| Step | Command | Fails when |
|---|---|---|
| 1 | `cargo fmt --check` | A file is not formatted. Fix it with `cargo fmt`. |
| 2 | `cargo clippy -- -D warnings` | Any lint fires. Warnings are errors. |
| 3 | `cargo test --verbose --tests -- --test-threads=1` | A test fails, or a test target does not compile. |

Formatting is checked first, so a formatting failure hides lint and test failures until it is fixed. Run `cargo fmt` before you start.

Each crate reports its own problems, so the same formatting or lint message can appear twice. The reason is in [Architecture](dev-architecture.md#crates-and-entry-points).

Tests run on one thread because several of them set process-wide environment variables such as `HOME` and `XDG_*`, and because GTK may only be used from one thread.

## Test layout

| Location | Contents |
|---|---|
| `#[cfg(test)]` modules in `src/` | Unit tests that sit next to the code and can see private items. This includes the documentation sync tests. |
| `tests/main_test.rs` | Command-line parsing: `resolve_startup_action` and `StartupAction`. |
| `tests/model_test.rs`, `tests/test_security.rs` | Model types and security checks. |
| `tests/hwga_regression_test.rs` | Call-count regressions, in debug builds only. |
| `tests/services.rs` and `tests/services/` | Loader, archives, database, search, task queue, thumbnails, terminal emulator and others. |
| `tests/ui.rs` and `tests/ui/` | Components, dialogs, menus, keymap, settings, navigation and update routing. |
| `tests/utils.rs` and `tests/utils/` | Config, search parsers, glob, helpers, media and xattr. |

`tests/services.rs`, `tests/ui.rs` and `tests/utils.rs` are each one test target that lists its files with `mod` lines. **A new file under those folders only runs after you add a `mod` line for it.**

Integration tests link against the library crate, so they use the `flux::` paths. They can only reach public items.

## Running part of the suite

```bash
cargo test --lib                                   # unit tests only
cargo test --lib utils::search::                   # one module
cargo test --lib doc_is_in_sync                    # the documentation checks
cargo test --test services terminal_emulator       # one target, filtered by name
cargo test --test ui components_test -- --test-threads=1
```

Tests that build widgets call `gtk::init()`. Several skip themselves when it fails, but run the suite under a display, either a real session or a virtual one such as `xvfb-run`, so the widget tests actually run.

## Documentation checks

`cargo run --bin gen-docs` regenerates the derived files. Each has a test that compares the committed copy with the generated text and fails with a message that names the file.

| Test | Guards |
|---|---|
| `doc_cli::tests::doc_is_in_sync` | `docs/cli.md` |
| `model::doc_config::tests::doc_is_in_sync` | `docs/config.md` |
| `utils::search::tests::doc_is_in_sync` | `docs/search.md` |
| `ui::doc_css_variables::tests::doc_is_in_sync` | `docs/css-variables.md` |
| the menu editor's sync test | `docs/actions.md` |
| `doc_fish::tests::completions_are_in_sync` | `completions/flux-fm.fish` |
| `doc_fish::tests::integration_is_in_sync` | `data/fish/flux-integration.fish` |

CI runs `cargo run --bin gen-docs` and then `git diff --exit-code docs/`, so a stale file fails the build even without the tests. Never edit a generated file by hand.

Some changes that look unrelated touch generated files:

- **Using a custom property in `src/ui/style.css`** changes its use count in `docs/css-variables.md`.
- **Declaring a new `--property`** needs an entry in `CUSTOM_PROPERTY_NOTES` in `src/ui/doc_css_variables.rs`.
- **Using a new `@color`** needs an entry in `GTK_COLORS` or `PALETTE_COLORS` there. A color that is not defined by GTK or libadwaita, such as an old palette name, is a bug in the stylesheet, not a missing table row.
- **Adding a command-line option** changes `docs/cli.md`, the fish completions and the `--help` text together, and a test checks that every option has a completion.

## Common failures

| Symptom | Likely cause |
|---|---|
| `Diff in <file>` | A formatting problem. Run `cargo fmt`. |
| A `clippy` error with `-D warnings` | Fix the lint, or add a narrowly scoped `#[allow(...)]` with a comment explaining why. |
| `could not compile ... (test "ui")` | A test no longer matches a refactored type, for example a struct that gained a field. Update the test. |
| `<file> is stale. Run cargo run --bin gen-docs` | A source change altered generated output. Regenerate and commit the result. |
| A test passes alone and fails in the suite | It depends on process state such as environment variables or a global cache. Keep it on one thread and reset what it changes. |
| A new test never runs | The file is missing from the `mod` list in `tests/ui.rs`, `tests/services.rs` or `tests/utils.rs`. |

Cargo stops at the first target that fails to compile or fails a test, so fixing one failure can reveal the next. Rerun `sh tools/tests.sh` until it passes.

## Fuzzing

Archive backends are fuzzed through the targets in `fuzz/fuzz_targets/`. See [Adding an archive backend](dev-archive-backend.md). The build script skips its git and asset steps when `cargo-fuzz` sets `CARGO_CFG_FUZZING`.
