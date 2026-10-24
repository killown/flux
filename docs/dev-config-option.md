# Adding a configuration option

Settings live in `config.toml` and are modelled by `UIConfig`
(`src/domain/model/config/ui.rs`). Most options follow the same path from the
file to a switch in the settings window. This guide follows a boolean,
`ui.single_click`, as the worked example.

## Steps

1. **`src/domain/model/config/ui.rs`** - add a doc-commented field to
   `UIConfig`:

   ```rust
   /// Whether a single click activates an item instead of a double click.
   pub single_click: bool,
   ```

   The struct is `#[serde(default)]`, so old `config.toml` files without the
   key keep loading. A field whose default is not the type's zero value needs
   `#[serde(default = "default_<name>")]` and a helper in
   `src/domain/model/defaults.rs`. Add the field to `impl Default for UIConfig`
   in the same file with the same value.

2. **`src/domain/model/doc_config.rs`** - add a one-line description to
   `FIELD_NOTES`:

   ```rust
   ("ui.single_click", "Activate items with a single click."),
   ```

   Tests in that file fail if a key has no note, or a note names a key that
   does not exist. Keys that are unset by default (for example
   `ui.theme`) go in `OPTIONAL_KEYS` instead.

3. **`src/utils/config/defaults.rs`** - the generator for the first-run
   `config.toml` and its legacy-key migration. Add the key to the generated
   text and to the struct literal in that file if the option should appear in
   a fresh config.

4. **`src/domain/model/msg.rs`** - add the setter variant, for example
   `SetSingleClick(bool)`. See [Adding a message](dev-message-bus.md).

5. **`src/app/update/settings/`** - add the handler to the file for the
   section (`behavior.rs`, `appearance.rs`, `view.rs`, `thumbnails.rs`, ...).
   It updates `self.config`, applies the change to live widgets, and calls
   `utils::save_config`:

   ```rust
   pub fn handle_set_single_click(&mut self, val: bool) {
       self.config.ui.single_click = val;
       self.files.view.set_single_click_activate(val);
       utils::save_config(&self.config);
   }
   ```

6. **The router** - add the arm in the router that covers the area (the
   `SetSingleClick` arm is in `src/app/update/view/mod.rs`).

7. **`src/ui/settings/page/<section>.rs`** - add the row. The widget sends
   the setter message through `model::SENDER`:

   ```rust
   add = &adw::SwitchRow {
       set_title: &tr("Single Click to Open"),
       set_active: config.ui.single_click,
       connect_active_notify => move |switch| {
           if let Some(s) = crate::model::SENDER.get() {
               let _ = s.send(AppMsg::SetSingleClick(switch.is_active()));
           }
       }
   },
   ```

   Wrap user-visible text in `tr()` so it reaches the `po/` files.

8. **Regenerate the docs** - `cargo run --bin gen-docs` rewrites
   `docs/config.md`. CI fails if the committed copy is stale. See
   [Testing and CI](dev-testing.md).

## Notes

- `save_config` primes the cached config. A watcher in `src/cli/watcher.rs`
  reloads the file when it changes on disk, so hand edits apply to a running
  window.
- Options read on a hot path (thumbnails, icons) go through the lock-free
  snapshots in `src/utils/config/snapshots.rs` so that no `RwLock` is taken
  per item. Add the field to the snapshot if you read it there.
- A renamed key needs an entry in `MIGRATIONS` in `doc_config.rs`, plus the
  matching migration in `src/utils/config/defaults.rs`.
