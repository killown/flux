# Command line

```text
flux [OPTIONS] [PATH]
```

The binary is built as `flux-fm`; `--help` refers to it as `flux`. `PATH` may be a plain path or a `file://` URI; the prefix is stripped and the rest is not percent-decoded.

## Opening a location

| `PATH` | Result |
|---|---|
| none | Opens the home directory. |
| a directory | Opens it. |
| a supported archive file | Opens the archive explorer on it. |
| `#name`, `:tag:name`, `:t:name` or `tags://name` | Opens the home directory filtered by the tag `name`. |

## Options

<!-- BEGIN GENERATED: options -->
| Option | Description |
|---|---|
| `-h, --help` | Print this help message |
| `-v, --version` | Print version information |
| `--no-sidebar` | Disable sidebar for this session |
| `--no-header` | Disable header bar for this session |
| `--no-statusbar` | Disable statusbar for this session |
| `--file-properties PATH` | Open the file properties window for PATH |
| `--set-icon TARGET IMAGE` | Set a custom icon/image for TARGET |
| `--cwd PATH` | Navigate the running window to PATH (starts a new window if none is running) |
| `--set-icons-stdin` | Read TARGET&lt;TAB&gt;IMAGE pairs from standard input |
| `--reset-icon TARGET` | Reset custom icon for TARGET back to default |
| `--reset-icons-stdin` | Read TARGET paths from standard input to reset |
| `--quick-list PATH...` | Open with quick-panel pre-populated with PATHs |
| `--quick-list-stdin` | Open with quick-panel populated from standard input |
| `--preview PATH` | Open folder PATH with the preview panel in wide mode |
| `--selection` | Print the paths selected in the running window, one per line (-0: NUL-separated) |
| `--toggle` | Show or hide the running window |
| `--show` | Bring the running window to the front |
| `--hide` | Hide the running window |
| `--theme NAME` | Set the theme to NAME, or 'default' to remove it |
| `--menu-editor` | To manage menu.rs |
<!-- END GENERATED: options -->

## Session flags

`--no-sidebar`, `--no-header` and `--no-statusbar` may appear anywhere on the command line. They apply to the window opened by `PATH`, a tag, `--preview`, `--quick-list` or `--quick-list-stdin`, and are ignored by the other options.

Every other option must be the first argument once the session flags are removed. Arguments after the first are read only by the options that take values.

## Custom icons

`--set-icon` stores the icon in `config.toml`. For a directory it also updates any matching pinned sidebar entry. `TARGET` and `IMAGE` are resolved to canonical absolute paths, and the command fails if `IMAGE` does not exist.

`--set-icons-stdin` reads one pair per line. Separate the two paths with a tab, or write two words and quote or escape any that contain spaces. Blank lines and lines starting with `#` are ignored. A bad line is reported on stderr with its line number and skipped, and the command exits with status 1 after applying the rest.

```text
/home/me/Music /home/me/icons/music.svg
"/home/me/My Videos" /home/me/icons/video.svg
```

`--reset-icons-stdin` reads one target path per line, with the same rules for blank lines and comments.

## Quick list

`--quick-list` opens the first path and fills the quick panel with all the paths given. Arguments that start with `-` are not treated as paths. `--quick-list-stdin` reads one path per line, skipping blank lines and `#` comments, and opens the home directory when the input is empty.

```text
find . -name '*.log' | flux --quick-list-stdin
```

## Selection

`--selection` asks a running window for the paths selected in its active tab and prints them, one per line. It talks to the window named by `$FLUX_PID`, which Flux sets in its embedded terminal, and otherwise to the window that navigated last. It does not open a window.

A path containing a newline is skipped with a warning; `--selection -0` terminates each path with NUL instead, which keeps every path intact for `xargs -0`.

```text
flux-fm --selection | xargs -d '\n' ls -l
```

## Window visibility

`--toggle`, `--show` and `--hide` take no arguments and control a window that is already running. `--toggle` flips its visibility, `--show` makes it visible and brings it to the front, and `--hide` hides it. `--show` and `--hide` are idempotent.

Each window listens on `$XDG_RUNTIME_DIR/flux/toggle-<pid>.sock`, or under `$XDG_CACHE_HOME/flux/` when there is no runtime directory. The command connects to the window with the highest PID, sends one line (`toggle`, `show` or `hide`) and reads one line back, `visible` or `hidden`, which is the state the window ended up in.

When no window is running, `--toggle` and `--show` open the home directory as if no arguments were given, and `--hide` does nothing and exits with status `0`.

```text
flux-fm --toggle
```

## Theme

`--theme NAME` stores `NAME` as the theme in `config.toml` and exits without opening a window. Running windows apply it as soon as the file changes. `NAME` is the file name of an installed theme without `.css`, such as one in `~/.local/share/flux/themes`; `default` removes the theme. An unknown name lists the available themes on stderr and exits with status `1`.

```text
flux-fm --theme default
```

## Exit status

| Status | Meaning |
|---|---|
| `0` | Success. Missing values for `--file-properties`, `--set-icon`, `--reset-icon`, `--theme` and `--quick-list` print the help text and also exit with `0`. |
| `1` | Unrecognized option, `--preview` without a directory, an icon file that does not exist, a skipped line in `--set-icons-stdin`, an unknown `--theme` name, or `--selection` finding no running window. |
