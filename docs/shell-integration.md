# Shell integration

A shell can ask a running Flux window what is selected and which folder it shows, and it can tell the window to navigate. The command-line pieces work in any shell. A set of fish helpers is built on top of them.

## In any shell

| Command | Result |
|---|---|
| `flux-fm --selection` | Prints the paths selected in the active tab, one per line. `-0` ends each path with NUL instead. |
| `flux-fm --cwd PATH` | Navigates the running window to `PATH`. Starts a window if none is running. |
| `flux-fm --toggle`, `--show`, `--hide` | Show or hide the running window. |

`--selection` needs no display and talks to the window named by `$FLUX_PID`, which Flux sets in its embedded terminal. Outside it, the window that navigated last answers. See [Command line](cli.md) for the exit statuses.

```bash
# Long listing of the selected files
flux-fm --selection | xargs -d '\n' ls -l

# Safe for any file name, including ones with a newline
flux-fm --selection -0 | xargs -0 tar -cf selection.tar

# Open a folder in the running window
flux-fm --cwd ~/Projects
```

A path that contains a newline is skipped with a warning unless you use `-0`.

## Fish helpers

| Helper | Description |
|---|---|
| `flux_selection` | Prints the selected paths, one per line. `-n` or `--names` prints base names. `-d` or `--dir` prints the folders that contain them, sorted and unique. Prints nothing when no window is running or nothing is selected. |
| `flux_cwd` | Prints the folder shown in the grid. Archives, Trash, Recents and network locations are skipped, so you get the last real folder. |
| `flux_goto [PATH]` | Navigates the open window to `PATH`, default `.`. A file means its parent folder. If no live Flux window is found it starts one at that folder. Fails with status `1` when the path does not exist. |
| `$F` | Abbreviation that expands to `(flux_selection)` as you type, in any position on the line. |
| `$FN` | Expands to `(flux_selection --names)`. |
| `$FD` | Expands to `(flux_selection --dir)`. |
| `$FCWD` | Environment variable set before every command to the folder shown in the grid. |
| `$FLUX_SELECTION` | Environment variable holding the escaped selection, set only for a command line that mentions `FLUX_SELECTION`. |

Because the abbreviations expand in the command line before you press Enter, you see the real paths first.

```fish
cp $F ~/backup/        # copy what is selected in Flux
ls -l $FN              # names only
cd (flux_cwd)          # follow the folder Flux shows
flux_goto ~/Downloads  # make Flux follow you
```

## Where the helpers are available

**Embedded terminal.** `F4` opens the terminal in the position set under `[ui.terminal]`. The shell is `ui.terminal.shell`, else `$SHELL`, else `/bin/bash`. The helpers need fish.

The embedded terminal only defines the helpers when `ui.fish_theme` is `true`. They come from the generated `~/.config/fish/conf.d/flux-theme.fish`, and Flux writes that file and exports its `FLUX_THEME_*` variables only when the option is on. The option is off by default.

**Any terminal.** Install the integration file. It is generated into `data/fish/flux-integration.fish` in the source tree and defines the same helpers:

```sh
cp data/fish/flux-integration.fish ~/.config/fish/conf.d/flux-integration.fish
```

You can also `source` it from `config.fish`. Loading it together with the theme snippet only redefines the same functions.

`completions/flux-fm.fish` completes every option, for `flux-fm` and for `flux`. Copy it to `~/.config/fish/completions/`.

## Matching the embedded prompt to the theme

With `ui.fish_theme = true`, Flux writes `~/.config/fish/conf.d/flux-theme.fish` and rewrites it on every theme change. It maps the GTK theme colors to fish's `fish_color_*` variables and redefines `fish_prompt` so the `❯❯❯` glyph follows the theme. Every line is guarded by `FLUX_THEME_FG`, so the file does nothing outside Flux's terminal.

If a `flux-theme.fish` that Flux did not write already exists, it is renamed to `flux-theme.fish.bak` once and never overwritten again. Turning the option off stops Flux from exporting the theme variables, so shells started afterwards are not themed.

## How it works

The shell side is plain files and sockets, so you can use them from any language.

| Path | Purpose |
|---|---|
| `$XDG_CACHE_HOME/flux/cwd` | The folder shown in the window that navigated last. `XDG_CACHE_HOME` defaults to `~/.cache`. |
| `$XDG_CACHE_HOME/flux/pid` | The process id of that window. |
| `$XDG_CACHE_HOME/flux/goto` | A shell writes a folder here. The window named in `pid` navigates to it and clears the file, so a request is never replayed. |
| `$XDG_RUNTIME_DIR/flux/selection-<pid>.sock` | A window answers a connection with its selection, each path ending in NUL. Falls back to the cache folder when there is no runtime folder. |
| `$XDG_CACHE_HOME/flux/ipc.sock` | Serves `--cwd`. The folder is `0700`, the socket `0600`, and the client checks that the server runs as the same user. |

Selection is not cached. The window resolves it on its main thread when asked, so the answer always matches what you see.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `$F` expands to nothing | No window is running or nothing is selected. Run `flux-fm --selection`. It prints `flux: no running window` or `flux: window N is not answering` and exits with `1`. |
| `$F` or `flux_goto` is undefined in the embedded terminal | Turn on `ui.fish_theme`, or install `flux-integration.fish`. |
| Another window answers | With several windows, the one that navigated last answers, except in Flux's own terminal, where `$FLUX_PID` pins the window. |
| `flux_goto` opens a new window | The `pid` file is stale or names a process that is not `flux-fm`. Navigate once in the window you want to refresh it. |
