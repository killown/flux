# Adding a message (AppMsg variant)

Every event in Flux is an `AppMsg` (`src/domain/model/msg.rs`). Widgets and
worker threads send one with `sender.input(AppMsg::...)`, and
`FluxApp::handle_update` (`src/app/update/mod.rs`) delivers it. Read
[Architecture](dev-architecture.md#message-flow) first for the overview.

## The routing contract

`handle_update` tries the routers in this order through the `route!` macro:
`navigation`, `loading`, `view`, `thumbnails`, `search`, `file_ops`,
`dialogs`, `context_menu`, `network`, `git`, `window`, `sidebar`.

Each router has the signature

```rust
pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg>
```

It matches the variants it owns, calls a `handle_*` method on `FluxApp`, and
ends with `other => return Err(other)`. `Err` passes the message to the next
router. A message that no router claims is logged as
`[flux] unhandled AppMsg` and otherwise ignored.

**The compiler does not check this.** A new variant without a router arm
compiles and runs, and does nothing. Always add the variant, the arm and the
handler together.

## Steps

1. **`src/domain/model/msg.rs`** - add the variant, doc-commented. Name
   result messages after what happened (`<X>Succeeded` / `<X>Failed`, or
   `<X>Ready`) and prompts `Prompt<X>`.

2. **A router under `src/app/update/`** - add one arm to the `match` in the
   router that owns the area. Pick the router by neighbours: search for a
   sibling variant with `grep -F 'variant AppMsg::<Sibling>' refs.txt`, or read
   the module list in `src/app/update/mod.rs`. The arm should be one line
   that calls a handler:

   ```rust
   AppMsg::SetSingleClick(val) => app.handle_set_single_click(val),
   ```

3. **The handler** - an `impl FluxApp` method named `handle_*` in the same
   directory tree (for example `src/app/update/settings/behavior.rs`). Mutate
   state and update widgets here. Keep it fast.

4. **Heavy work** - the handler spawns it (`relm4::spawn_blocking`,
   `tokio::spawn`, or the loader pool) and the worker reports back with a
   second variant through `sender.input(...)`. Background threads without a
   component sender use `infra::SENDER`. For work tied to the current folder,
   capture `load_id` when you spawn and discard the result if
   `self.load_id` has moved on (see `handle_folder_loaded_chunk`).

5. **Callers** - add the `sender.input(AppMsg::...)` call where the event
   starts: a widget in `src/ui/`, an input controller in `src/ui/inputs/`, or a
   worker in `src/infra/`.

## Renaming or removing a variant

Find every use first. In a Flux context bundle,
`grep -F 'variant AppMsg::Name' refs.txt` lists where the variant is matched
and built. Without a bundle, use `rg 'AppMsg::Name\b'`. Patch every site.

## Checklist

- The variant has a `///` comment.
- Exactly one router has an arm for it.
- The handler does no file I/O, hashing or process spawning on the main
  thread.
- `cargo run --bin gen-docs` shows no change, unless the message belongs to a
  documented feature.
