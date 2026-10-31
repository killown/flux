# Adding a Right Panel

Every right-side panel in Flux, Tag, Search, Diff, Location, Connect,
Preview, Actions, QuickList, Terminal, Properties, is wired through the same
small set of files. Their session state lives in one place,
`Panels` (`src/app/state/panels.rs`), held by `FluxApp` as `panels`.
The terminal panel (`src/ui/panels/terminal.rs` and the
`RightPanelType::Terminal` arms in `src/app/update/helpers/sidebar.rs`) is the
smallest complete example, because it exercises every hook: config flag,
message variants, lazy build, spawn-on-reveal, kill-on-hide, sidebar
stash, focus skip. Read those two files before starting.

## How the pieces fit

- **`Panels`** has one `PanelState` per panel: its `revealer`, `visible`,
  `initialized`, and `extras` (handles only some panels need, such as the
  terminal or the session tokens). It holds *session* state only.
  Persisted settings such as `config.ui.<panel>_width` stay in the config.
- **`Panels::close_all_except(keep)`** enforces "one panel at a time".
  Afterwards `keep` may be open and every other panel is hidden. It hides
  revealers and clears `visible`, leaves `initialized` alone (panels are built
  once and reused), and returns the kinds it closed. It never kills
  subprocesses, stops media or restores sidebars; that is what the close
  hooks are for.
- **`toggle_sidebar_right_panel`** (`sidebar.rs`) is the only entry point. It
  calls `close_all_except`, runs `on_panel_close` for each panel that was
  displaced, builds the panel on first use with `build_panel`, flips
  visibility, then runs `on_panel_open` or `on_panel_close`.
- **`build_panel`, `on_panel_open`, `on_panel_close`** are `match`es on
  `RightPanelType`. Most arms are one or two lines.

## Checklist

Work through these in order. Each line names the file, the change, and
_why_ that file is on the list. Missing any of them is either a compile
error or a silent bug.

1. **Config** - in `src/domain/model/config/ui.rs` add the enable flag
   (`bool` with `#[serde(default)]`) and, if the panel has a width or height,
   an `i32` field defaulted via a helper in `src/domain/model/defaults.rs`.
   Mirror the field in the parse-failure `UIConfig { ... }` literal in
   `src/utils/config/defaults.rs`; both branches must stay in sync or a
   corrupt `config.toml` silently drops the field. Then run
   `cargo run --bin gen-docs` for `docs/config.md`. Do not edit it by hand.

2. **`src/domain/model/types.rs`** - add a variant to `RightPanelType`,
   grouped with the other panel variants.

3. **`src/app/state/panels.rs`** - add a `PanelState` field to `Panels` and
   an arm for it in `get`, `get_mut`, `iter_mut` and `any_visible`. The
   `get`/`get_mut` matches are exhaustive, so the compiler flags a missing
   arm; `iter_mut` and `any_visible` are lists and it does not. `iter_mut` is
   also the order panels are closed in. If the panel needs handles of its own
   (a subprocess, a text buffer, a session token), add them to `PanelExtras`
   with a doc comment. There is nothing to add to `FluxApp` or its struct
   literal in `src/ui/startup/components.rs`.

4. **Revealer** - in `src/ui/view/component.rs` add a revealer to the `view!`
   macro as
   `#[name = "<panel>_panel_revealer"] panel_revealer(Some(PANEL_TRANSITION_MS)) {}`.
   The helper in `src/ui/view/pages/panels.rs` sets `SlideLeft`,
   `reveal_child: false` and `visible: false`. Match the neighboring panels.
   Then in `src/ui/view/init.rs`, in `store_widget_refs`, store it with
   `model.panels.<panel>.revealer = Some(widgets.<panel>_panel_revealer.clone())`.
   If the panel owns a subprocess (shell, mpv), add a
   `root.connect_close_request` that tears it down, mirroring the terminal
   block.

5. **Panel builder** - in `src/ui/panels/<panel>.rs` expose
   `pub fn build_<panel>_panel(initial_width: i32, sender: AsyncComponentSender<FluxApp>) -> gtk::Box`.
   Use `header::panel_header`, `resize::resizable_panel`, and
   `spec::PanelSpec`. Add `const fn <panel>()` to `spec.rs` with the
   min / max / default widths. Register it in `src/ui/panels/mod.rs` with
   `pub mod <panel>;` and `pub use <panel>::build_<panel>_panel;`.

6. **`src/app/update/helpers/sidebar.rs`** - add one arm to each of the three
   hook `match`es. They are exhaustive, so the compiler flags a missing arm:
   - `build_panel`: return the `gtk::Box` from `build_<panel>_panel(...)`,
     and store any handle the builder returns in `self.panels.<panel>.extras`;
   - `on_panel_open`: for a panel that needs text entry (Location, Connect,
     Search) call `focus_first_entry(revealer)`. Otherwise leave focus alone
     so arrow keys keep driving the grid;
   - `on_panel_close`: undo whatever `on_panel_open` did (restore the
     sidebar, kill the process, reset the panel). An empty arm is fine.

7. **Messages and translations** - in `src/domain/model/msg.rs` and
   `src/app/update/**/mod.rs`, if the panel has its own `AppMsg` variants, add
   them doc-commented and route them in the closest domain file. Put new
   strings in `po/pt_BR.po`, grouped under `# src/ui/panels/<panel>.rs`.

## Lifecycle rules

Panels are lazy: the widget tree is built on first open and kept alive
afterwards. These six rules are what keeps close/reopen correct.

- **One panel at a time.** `Panels::close_all_except` does the hiding, and
  `toggle_sidebar_right_panel` calls it first, so a new panel gets this for
  free once it has its `iter_mut` arm. Anything a displaced panel must undo
  goes in its `on_panel_close` arm. `on_panel_close` is told whether the panel
  was toggled shut or displaced by another one (`CloseReason`), because a few
  hooks only apply to one of the two.

- **Spawn on reveal, kill on hide.** Subprocesses (shell, media player)
  are spawned in `on_panel_open` and killed in `on_panel_close`. Do not reuse
  a process across toggles - the fd, pipe, or socket will be stale.

- **Do not grab focus by default.** Preview, Actions, QuickList, Terminal and
  Properties do not call `focus_first_entry` in `on_panel_open`. Do the same
  unless the panel's purpose is text entry (Tag, Search, Diff, Location,
  Connect). Grabbing focus breaks arrow-key scrubbing in the grid.

- **Stash the left sidebar if the panel is wide.** Terminal and QuickList do
  this: store `Some(self.sidebar_visible)` in the panel's `extras` on open,
  restore it in `on_panel_close`. Preview does the same through
  `wide_preview_sidebar`. Do not touch `config.ui.sidebar_visible` - that is
  a persisted preference, not session state.

- **Session-token every async load.** Bump an `Arc<AtomicU64>` kept in the
  panel's `extras` on open, capture its value, drop results whose token no
  longer matches. This is what prevents old contents from flashing into a
  newly opened panel on rapid switching.

- **Stop media on close.** If the panel plays video or audio, call
  `stop_media()` (or equivalent) in `on_panel_close`. Otherwise the
  GStreamer pipeline and its fds keep running in the background.

## Verifying the result

Once wired up, the panel should:

- open and close on `F<key>` without touching any other panel's state;
- survive a config reload (`F10` on the settings window, then close it)
  with its flag preserved;
- keep the file grid responsive to arrow keys while it is open;
- leave no subprocess behind after the window closes
  (`ps -ef | grep -E 'mvp|sh -l'` should not show orphans);
- not flash stale contents when re-opened after the underlying data
  changed on disk.
