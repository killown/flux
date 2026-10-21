# Adding a Keybinding

Flux has two shortcut systems that look identical from the user's
perspective but are wired differently:

- **Configurable shortcuts** - `config.shortcuts.<name>` in
  `~/.config/flux/config.toml`. Resolved through `KeyMap` and used by
  the help window. Used for the actions a user might reasonably want to
  rebind (open, delete, refresh, toggle hidden, tab management).
- **Hardcoded shortcuts** - F-keys, `Ctrl+Shift+<letter>` chords for
  panels, `Ctrl+Alt+<letter>` folder cycle. Registered directly in
  `src/ui/inputs/capture.rs` or `src/ui/inputs/shortcuts.rs` because
  they are structural (they toggle a panel, cycle the folder prefix,
  etc.) and rebinding them would break the mental model.

Pick the configurable system unless the shortcut is a fixed panel toggle
or a cycle. The rest of this guide is the configurable path, the
hardcoded path is a single match arm in `capture.rs` and has no other
steps.

## Checklist

1. **`src/model/config/shortcuts.rs`** - add an `Option<String>` field
   to `ShortcutsConfig`, doc-commented with the default accelerator
   (`"<Primary>k"`, `"F7"`, etc.). If the default is a stable contract
   (like `open = "Return"`), spell it out in the comment.

2. **`src/ui/keymap/constants.rs`** - add a `pub const <NAME>: &str`
   for the fallback accelerator. This is what `KeyMap::new` uses when
   `config.shortcuts.<name>` is `None` or `Some("")`.

3. **`src/ui/keymap/map.rs`** - add a `pub <name>: gtk::ShortcutTrigger`
   field to `KeyMap` and initialise it in `KeyMap::new` with
   `parse_trigger(&config.<name>, constants::<NAME>)`. This is the only
   place the fallback chain lives.

4. **`src/ui/inputs/shortcuts.rs`** - register the trigger with the
   controller. Most shortcuts go through the `bind_msg!(keymap.<name>,
AppMsg::<X>)` macro at the top of `setup_global_shortcuts`. If the
   shortcut has a guard (skip when an entry is focused, yield to the
   terminal), follow the `search` pattern: a separate
   `ShortcutController` with a `CallbackAction` that checks
   `root.focus()` before dispatching.

5. **`src/ui/help/page/*.rs`** - add a `row(&tr("Description"),
&sc(config, |s| s.<name>.clone(), "<default>"))` to the relevant
   page. The `sc` helper formats the accelerator for the keycap label
   (`Ctrl + K`, not `<Primary>k`).

6. **`po/pt_BR.po`** - the human-readable description and any dialog
   title. The accelerator itself is not translated.

## When the shortcut needs a guard

A shortcut that should not fire while a text entry is focused needs
the `ShortcutController` shape, not `bind_msg!`:

    let sc = gtk::ShortcutController::new();
    sc.set_scope(gtk::ShortcutScope::Managed);
    sc.add_shortcut(gtk::Shortcut::new(
        Some(keymap.<name>.clone()),
        Some(gtk::CallbackAction::new(move |widget, _| {
            let rename_focused = widget
                .root()
                .and_then(|root| root.focus())
                .map(|focused| {
                    focused
                        .css_classes()
                        .iter()
                        .any(|c| c.as_str() == "flux-rename-entry")
                })
                .unwrap_or(false);

            if rename_focused {
                return glib::Propagation::Proceed;
            }

            sender.input(AppMsg::<X>);
            glib::Propagation::Stop
        })),
    ));
    window.add_controller(sc);

`Scope::Managed` is required. `Scope::Global` fires before the focused
widget's own controller and breaks typing into entries. Use `Managed`
for everything user-facing.

## Where the reference code lives

- `src/ui/keymap/map.rs` and `src/ui/keymap/parse.rs` - the fallback
  chain (`user config → constant → "Escape"`). Read these before adding
  a field a malformed user value must degrade to the constant, not
  panic.
- `src/ui/inputs/shortcuts.rs` - the `bind_msg!` macro and the
  `bind_trigger!` macro for literal triggers. `setup_settings_shortcut`
  shows the `Managed`-scope-with-guard pattern.
- `src/ui/inputs/capture.rs` - the hardcoded shortcuts. This file
  yields to the terminal and to editable widgets. If you add a chord
  here, you must yield in the same places (see `terminal_focused` and
  `is_editable`).
- `src/ui/help/page/navigation.rs` - the `sc(...)` helper usage that
  renders a config value into a keycap label.

## Behaviour rules

- **`Managed` scope, not `Global`.** Global fires before the focused
  widget's controller and breaks typing. The only legitimate use of
  `Global` is a shortcut that must work no matter what - there should
  be none in this codebase.
- **Yield to the terminal.** Both `capture.rs` and `shortcuts.rs` check
  `flux-terminal-area` on the focused widget. A shortcut that fires
  while the shell has focus breaks shell completion, `Ctrl+C`, `Tab`,
  and every other key the terminal owns.
- **Yield to editable widgets.** If the focused widget is a
  `gtk::Editable`, `Ctrl+A`, `Ctrl+C`, `Ctrl+V`, `Ctrl+Z` belong to the
  entry, not the app. Check `is_editable` before dispatching.
- **Fallback chain is `config → constant → "Escape"`.** A user-supplied
  accelerator that GTK refuses to parse must not leave the shortcut
  unbound. `parse_trigger` handles this, do not bypass it.
- **A rebindable shortcut needs a help-window row.** If it is not in
  the help window, the user has no way to discover or verify the
  rebind. Hardcoded shortcuts go in `system_view.rs`, configurable
  ones go in their topic page.
- **Do not add a shortcut that shadows `Space` or `Enter`.** Both are
  handled in `handle_key_event` with specific `is_editable` and
  `terminal_focused` guards that a new `ShortcutController` will not
  replicate.

## Verifying the result

After wiring up the shortcut, it should:

- fire when the grid has focus and nothing is typed
- not fire when a rename entry is focused, a search entry is focused,
  or the terminal has focus
- appear in the help window with the correct keycap label
- survive a rebind through `config.toml` (edit the file, reload the
  window with `F5`, verify the old key no longer fires and the new one
  does)
- fall back to the default when the user config contains garbage
  (e.g. `shortcuts.<name> = "<NotAKey>"` - the help window should show
  the default, and the default should fire)
- not break `Space`, `Enter`, `Tab`, `Escape`, or the terminal.
