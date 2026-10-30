# Adding a Right Panel

Every right-side panel in Flux, Tag, Search, Diff, Location, Connect,
Preview, Actions, Terminal, is wired through the same set of files.
The terminal panel (`src/ui/panels/terminal.rs` and the
`RightPanelType::Terminal` arms in `src/app/update/helpers/sidebar.rs`) is the
smallest complete example, because it exercises every hook: config flag,
message variants, lazy build, spawn-on-reveal, kill-on-hide, sidebar
stash, focus skip. Read those two files before starting.

## Checklist

Work through these in order. Each line names the file, the change, and
_why_ that file is on the list. Missing any of them is either a compile
error or a silent bug.

1. **`src/domain/model/config/ui.rs`** - add the enable flag (`bool` with
   `#[serde(default)]`). If the panel has a width or height, add an
   `i32` field defaulted via a helper in `src/domain/model/defaults.rs`.

2. **`src/utils/config/defaults.rs`** - mirror the field in the
   parse-failure `UIConfig { ... }` literal. Both branches must stay in
   sync or a corrupt `config.toml` silently drops the field.

3. **`docs/config.md`** - run `cargo run --bin gen-docs`. Do not edit by
   hand.

4. **`src/domain/model/types.rs`** - add a variant to `RightPanelType`, grouped
   with the other panel variants.

5. **`src/domain/model/app.rs`** - add three fields minimum:
   `Option<gtk::Revealer>`, `bool visible`, `bool initialized`. If the
   panel hides the left sidebar (Terminal, Preview), add `Option<bool>`
   to stash the prior visibility. Doc-comment each field so the
   `Debug` derive stays readable.

6. **`src/ui/startup/components.rs`** - initialize the new `FluxApp`
   fields in the struct literal. `None` / `false` unless the panel is
   open at startup.

7. **`src/ui/view/component.rs`** - add a revealer in the `view!` macro
   as `#[name = "<panel>_panel_revealer"] panel_revealer(Some(PANEL_TRANSITION_MS)) {}`.
   The helper in `src/ui/view/pages/panels.rs` sets `SlideLeft`,
   `reveal_child: false` and `visible: false`. Match the neighboring panels.

8. **`src/ui/view/init.rs`** - in `store_widget_refs`, store the
   revealer on the model. If the panel owns a subprocess (shell, mpv),
   add a `root.connect_close_request` that tears it down, mirroring the
   terminal block.

9. **`src/app/update/helpers/sidebar.rs`** - three edits inside
   `toggle_sidebar_right_panel`:
   - add the variant to `hide_other_right_panels` so opening another
     panel closes this one;
   - add a match arm in the `(revealer, initialized, visible, is_search)`
     tuple;
   - add a match arm in the panel-construction `match` that returns the
     `gtk::Box` from `build_<panel>_panel(...)`.

   If the panel needs focus (Location, Connect, Search), leave it out of
   `skip_focus`. Otherwise add it to `skip_focus` so arrow keys keep
   driving the grid.

10. **`src/ui/panels/mod.rs`** - `pub mod <panel>;` and
    `pub use <panel>::build_<panel>_panel;`.

11. **`src/ui/panels/<panel>.rs`** - expose
    `pub fn build_<panel>_panel(initial_width: i32, sender: AsyncComponentSender<FluxApp>) -> gtk::Box`.
    Use `header::panel_header`, `resize::resizable_panel`, and
    `spec::PanelSpec`. Add `const fn <panel>()` to `spec.rs` with the
    min / max / default widths.

12. **`src/domain/model/msg.rs`** and **`src/app/update/**/mod.rs`** - if the panel
    has its own `AppMsg` variants, add them doc-commented and route them
    in the closest domain file.

13. **`po/pt_BR.po`** - new strings, grouped under
    `# src/ui/panels/<panel>.rs`.

## Lifecycle rules

Panels are lazy: the widget tree is built on first open and kept alive
afterwards. These five rules are what keeps close/reopen correct.

- **Spawn on reveal, kill on hide.** Subprocesses (shell, media player)
  are spawned in the `now_visible` branch and killed in the
  `!now_visible` branch of `toggle_sidebar_right_panel`. Do not reuse a
  process across toggles - the fd, pipe, or socket will be stale.

- **Do not grab focus by default.** Preview, Actions and Terminal add
  themselves to `skip_focus`. Do the same unless the panel's purpose is
  text entry (Location, Connect, Search). Grabbing focus breaks
  arrow-key scrubbing in the grid.

- **Stash the left sidebar if the panel is wide.** Terminal and Preview
  do this: store `Some(self.sidebar_visible)` on first reveal, restore on
  hide. Do not touch `config.ui.sidebar_visible` - that is a persisted
  preference, not session state.

- **Session-token every async load.** Bump an `Arc<AtomicU64>` on open,
  capture its value, drop results whose token no longer matches. This is
  what prevents old contents from flashing into a newly opened panel on
  rapid switching.

- **Stop media on close.** If the panel plays video or audio, call
  `stop_media()` (or equivalent) in the `!now_visible` branch. Otherwise
  the GStreamer pipeline and its fds keep running in the background.

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
