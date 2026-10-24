# Adding a Menu Action

Flux has two context-menu surfaces:

- **Primary right-click** - built from `~/.config/flux/menu.rs` and
  `src/ui/menu/context.rs`. Every entry is a `CustomAction`. Shell
  commands run via `builtin::` dispatch, user commands go through
  `handle_execute_command`.
- **Secondary Ctrl+right-click** - built from
  `~/.config/flux/menus/menu-<category>-<subtype>.rs` and
  `src/ui/menu/secondary/`. Every entry is also a `CustomAction`, but
  the parsing, matching and rendering are separate code paths.

This guide is for **`builtin::` actions**, which are registered in the
first surface only. Adding a user-facing shell command to the default
`menu.rs` is a different procedure - append a line to
`src/utils/config/menu.rs::ensure_config_file` and add it to
`docs/actions.md` manually.

## Checklist

1. **`src/ui/menu/editor/builtins.rs`** - add a `BuiltinDef` to the
    `BUILTINS` table:

        BuiltinDef {
            name: "builtin::my_action",
            scope: "all",  // or "file", "directory", "image/all", ...
            desc: "One sentence describing what the action does.",
        },

    The `desc` field is rendered into the existing `docs/actions.md`
    table, and the `doc_is_in_sync` test in `src/doc_cli.rs` fails if
    the committed page drifts from the table. It is also shown verbatim
    in the "Pick Built-in" dialog in the menu editor.

2. **`src/ui/menu/context.rs`** - add the dispatch arm. The
    `match action.command.as_str() { ... }` block has three shapes. Pick
    the one that fits:

    - **Stateless, no target** - most actions. Use `register_simple`
      with a closure that dispatches an `AppMsg`:

      ```rust
      "builtin::my_action" => register_simple(
          &action_group,
          "my-action",
          sender,
          action_toast.clone(),
          |s, toast| {
              s.input(AppMsg::MyActionMsg);
              emit_toast(s, toast);
          },
      ),
      ```

    - **Path-bearing** - the action operates on the file or directory
      the menu was opened on. Use `register_path_action`:

      ```rust
      "builtin::my_action" => register_path_action(
          &action_group,
          "my-action",
          sender,
          target_path,
          action_toast.clone(),
          |s, target, toast| {
              s.input(AppMsg::MyActionMsg { path: target });
              emit_toast(s, toast);
          },
      ),
      ```

    - **Selection-based** - the action operates on the whole selection
      rather than the clicked item. Resolve `self.get_selection()` (or
      `get_selection_with_meta()` if you need `is_dir`) before the
      `register_simple` call, like `builtin::install_fonts` does.

3. **`src/domain/model/msg.rs`** - add the `AppMsg` variant(s) the dispatch
    arm produces, doc-commented. If the action opens a dialog, the
    message name is usually `Prompt<X>`, if it runs work in the
    background, `<X>Succeeded` / `<X>Failed` pair.

4. **`src/app/update/**/mod.rs`** - route the new `AppMsg` in the closest
    domain router (`file_ops/mod.rs`, `window/mod.rs`, `search/mod.rs`,
    `sidebar/mod.rs`, `git/mod.rs`, `network.rs`, `dialogs.rs`). Everything
    below the router is a `handle_*` method on `FluxApp`, living in
    `src/app/update/` or `src/ui/dialog/`.

5. **`docs/actions.md`** - **do not edit by hand**. Run
    `cargo run --bin gen-docs` to regenerate it from `BUILTINS`. The
    `gen-docs` binary produces the file, and `doc_is_in_sync` in
    `src/doc_cli.rs` (plus the matching test in the menu editor module)
    fails CI if the committed file drifts from the generated output.

6. **`po/pt_BR.po`** - any new user-facing string (toast, dialog title,
    error message). Group under `# src/ui/menu/context.rs` if the string
    originates there, otherwise under the handler file.

## Adding a user-visible menu entry to the default menu.rs

If the action should appear in the default context menu without the
user editing `menu.rs`, add a line to `ensure_config_file` in
`src/utils/config/menu.rs`:

    "󰀫      My Action" => "all", "builtin::my_action"

The leading glyph (before the first alphanumeric char) is a Nerd Font
icon. Every existing line in `default_config` uses one the menu editor
picks it up as a prefix. Do not add a leading space without a glyph,
the alignment breaks.

The line is only written on first run. Existing users will not see the
new entry until they delete `~/.config/flux/menu.rs`. Do not attempt to
migrate existing menus automatically - users edit `menu.rs` by hand,
and silently rewriting it destroys custom entries.

## Where the reference code lives

- `src/ui/menu/context.rs` - the dispatch table. Read the full
  `builtin::install_fonts` arm, which is the most complex
  selection-based case, and `builtin::open_new_tab`, which handles
  multi-tab activation.
- `src/ui/menu/editor/builtins.rs` - the `BUILTINS` table and the
  `render_actions_markdown()` renderer that produces the existing
  `docs/actions.md`.
- `src/app/update/file_ops/command.rs` - `handle_execute_command`, which is
  what non-`builtin::` entries dispatch to. If your action is closer
  to a shell command than a native operation, consider whether it
  needs a `builtin::` at all.
- `src/ui/menu/secondary/` - the Ctrl+right-click surface. Different
  procedure, every action here comes from a user template file, not
  from `BUILTINS`. Adding a `builtin::` to a secondary template works
  because `handle_execute_command` handles both surfaces uniformly.

## Behaviour rules

- **Never block the main thread.** If the action does any I/O, hashing,
  or subprocess spawn, dispatch a message to a handler and let the
  handler call `relm4::spawn_blocking` or `tokio::spawn`. The dispatch
  arm in `context.rs` must return in microseconds.
- **Respect the toast contract.** `emit_toast` fires whenever the
  `menu.rs` line has a third argument. Do not suppress it - users rely
  on it for feedback, and dropping it means a silent action.
- **Use `get_selection_with_meta` when the result depends on
  `is_dir`.** `get_selection` returns only paths, if the action
  branches on file vs directory, resolve the meta once at dispatch
  time rather than doing a `stat` inside the handler (which races with
  concurrent renames).
- **Skip the action when its precondition fails.** `context.rs` already
  does this for `%l` (line-number) actions and `builtin::tagfile`
  (which skips directories). Follow the same pattern: `continue` out of
  the loop if the target is unsupported, rather than registering an
  action that will no-op or error.
- **Do not reuse an action name across the two surfaces.** The
  `action_name` field in `CustomAction` is what the popover's
  `SimpleAction` registers against. Two entries with the same name will
  collide in `action_group` and the second registration wins.

## Verifying the result

After wiring up the action, it should:

- appear in the "Pick Built-in" dialog of the menu editor
- appear in `docs/actions.md` after `cargo run --bin gen-docs`, with the
  exact `desc` string from `BUILTINS`
- pass `cargo test --lib` (the `doc_is_in_sync` test will catch any
  drift between `BUILTINS` and the committed `docs/actions.md`)
- run from the primary right-click menu when an entry uses it
- not appear in the primary menu when its `mime_types` requirement
  doesn't match the current target
- show its toast immediately (for secondary-surface actions) or after
  the command exits (for shell commands in `menu.rs`), matching the
  contract documented in `docs/actions.md`.
