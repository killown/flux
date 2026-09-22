# 🌊 Flux

![Rust](https://img.shields.io/badge/Rust-1.75%2B-black?logo=rust)
![GTK4](https://img.shields.io/badge/GTK-4-green?logo=gnome)
![License](https://img.shields.io/badge/License-GPLv3-blue.svg)

**Flux** is the fastest modern GUI file manager on Linux. It combines a minimalist, high-performance architecture with the sleek aesthetic of Libadwaita, delivering a native desktop experience with zero compromise on speed.

<p align="center">
  <a href="https://www.youtube.com/watch?v=CUsf3Xyxw-k">
    <img src="https://github.com/killown/flux/raw/main/screenshots/flux.png" alt="Flux YouTube Showcase" width="100%">
  </a>
</p>

## Why Flux?

```bash
#CPU: AMD Ryzen 5 5600X (6 Cores, 12 Threads @ 3.7GHz Base / 4.6GHz Boost)
#Methodology: High-precision time.perf_counter() differential, IPC event stream.

# --- GUI File Managers ---
~ ❯❯❯ python measure_app_speed.py flux-fm
Startup Time: 112.55 ms    # cold: 114.57 ms | warm: 112.48 ms (±1.94 ms, 29 runs)

~ ❯❯❯ python measure_app_speed.py pcmanfm
Startup Time: 134.43 ms    # warm: 132.32 ms

~ ❯❯❯ python measure_app_speed.py spacefm
Startup Time: 169.31 ms    # warm: 169.15 ms

~ ❯❯❯ python measure_app_speed.py thunar
Startup Time: 289.60 ms    # warm: 182.37 ms

~ ❯❯❯ python measure_app_speed.py nautilus
Startup Time: 742.84 ms    # warm: 449.54 ms

# --- Terminal Emulators (Performance Baseline) ---
~ ❯❯❯ python measure_app_speed.py kitty
Startup Time: 107.45 ms    # warm: 107.29 ms

~ ❯❯❯ python measure_app_speed.py alacritty
Startup Time: 107.44 ms    # warm: 71.08 ms
```

- **Unrivaled Cold Launch:** The fastest modern GUI file manager on Linux-outperforming traditional lightweight C benchmarks like PCManFM and Thunar while launching up faster than mainstream alternatives, with zero background pre-warming daemons.
- **Asynchronous Heart:** Driven by a non-blocking Tokio and Rayon pipeline, chunked directory loading and background thumbnail workers ensure the UI thread never stutters or drops frames.
- **Zero Clutter:** A sleek, native Libadwaita interface with a dynamic header and adaptable grid/list views that prioritize your files over bulky window chrome.
- **Shortcut First:** Built from the ground up for keyboard-centric workflows, featuring vim-like speed, editable breadcrumb navigation, and an embedded VTE terminal that syncs directories on the fly.

---

## Features

### Navigation

- Autoplay video previews: automatically plays a loop preview when a single video file is selected.
- Dual view modes: responsive grid cards and compact list view
- Breadcrumb navigation supporting local paths, `/archive://` virtual roots, and network URIs
- Forward/backward history (`Alt+Left`, `Alt+Right`, `Backspace`, mouse side buttons)
- Automatic UI sync on filesystem changes via GIO directory monitors
- Configurable sorting by Name, Date, Size, or Type with Folders First toggle

### Search & Filtering

**Instant fuzzy filter** - powered by `nucleo` SIMD matching. Type anywhere to filter the current directory live with typo-tolerant subsequence scoring and zero UI stutter.

**Size filter** - mathematical operators directly in the search bar:

| Pattern      | Matches                       |
| :----------- | :---------------------------- |
| `>10MB`      | Files larger than 10 MB       |
| `<500KB`     | Files smaller than 500 KB     |
| `10MB..50MB` | Files between 10 MB and 50 MB |
| `>1GB video` | Size + name combined          |

Supported units: `B`, `KB`, `MB`, `GB`, `TB`.

**Deep content search** - prefix with `:` to search inside file bodies recursively:

- `:term` - search all files
- `:.rs:term` - scope to an extension
- `:.rs,py:term` - multiple extensions

**Session glob / MIME filter** - filter the current directory by pattern or type:

- `*.png`, `*.rs` - recursive extension globs
- `image/*`, `video/*`, `audio/*` - built-in category shorthands
- `application/zip`, `image/png` - any system MIME type (resolved via `/usr/share/mime/globs`)

**Advanced Search Sidebar (`Ctrl + F`)** - dedicated right panel featuring multi-threaded directory traversal via `ignore::WalkBuilder` with non-blocking batched results, combining name, content, extension, date range, size constraints, and recursion depth toggles.

### Built-in Actions

Flux provides internal routines that can be configured either via the built-in **Menu Editor** or directly inside `~/.config/flux/menu.rs` using the `builtin::` namespace.

Unlike shell actions, built-in commands run internal app routines without spawning an external process.

#### Reference

| Identifier                      | Description                                                                                                                 | Target Scope         |
| :------------------------------ | :-------------------------------------------------------------------------------------------------------------------------- | :------------------- |
| `builtin::inspect_dir`          | Opens the **Directory Inspector** modal (storage summary, top files, realtime search, enclosing folder jump, move to trash) | Directories          |
| `builtin::open_with`            | Dynamically populates a submenu containing registered system apps for the target MIME type                                  | Files                |
| `builtin::open_with_dialog`     | Opens the custom application chooser dialog                                                                                 | Files                |
| `builtin::copy`                 | Copies selected items to clipboard                                                                                          | Any                  |
| `builtin::cut`                  | Cuts selected items to clipboard                                                                                            | Any                  |
| `builtin::paste`                | Pastes clipboard items into the target/current folder                                                                       | Directories          |
| `builtin::rename`               | Enters inline renaming mode                                                                                                 | Any                  |
| `builtin::delete`               | Moves selected items directly to the system trash                                                                           | Any                  |
| `builtin::new_folder`           | Opens the batch folder creation dialog                                                                                      | Directories          |
| `builtin::new_file`             | Opens the batch file creation dialog                                                                                        | Directories          |
| `builtin::toggle_pin`           | Pins or unpins the folder in the sidebar                                                                                    | Directories          |
| `builtin::add_to_quick_list`    | Adds the item to the Quick List                                                                                             | Any                  |
| `builtin::quick_list_transfer`  | Opens a submenu to quickly move or copy selected items directly to any pinned Quick List destination                        | Any                  |
| `builtin::tagfile`              | Opens the tag manager modal for the selected file                                                                           | Files                |
| `builtin::set_custom_icon`      | Opens a file picker to assign a custom icon to an item                                                                      | Any                  |
| `builtin::reset_custom_icon`    | Restores an item's default system icon                                                                                      | Any                  |
| `builtin::set_extension_icon`   | Sets a global icon for the target's file extension                                                                          | Files with extension |
| `builtin::reset_extension_icon` | Restores the default icon for that file extension                                                                           | Files with extension |

#### Configuration Examples

**In `~/.config/flux/menu.rs`:**

```rust
"󰉋   Inspect Directory" => "directory", "builtin::inspect_dir"
"󰋜   Open With…"        => "all", "builtin::open_with_dialog"
"󰆴   Move to Trash"     => "all", "builtin::delete"
```

### Quick List / Triage Panel

Temporary pinned directory panel for fast multi-directory cycling:

- `Insert` - pin current path
- `Tab` / `Ctrl+PageDown` / `Ctrl+PageUp` - cycle entries
- `Ctrl+End` - clear

### File Operations

- Background task queue with speed, elapsed time, and ETA
- Conflict resolution dialog with auto-rename, Replace All, Skip All policies
- Undo/Redo (`Ctrl+Z` / `Ctrl+Shift+Z`) for renames, moves, and trash
- Clipboard: file lists, text-to-file, HTML, and image-to-file buffers
- Protected system path guards preventing accidental deletion of root/OS paths

### Context Menus & Custom Actions

- **Primary menu** - right-click
- **Secondary MIME-matched menu** - `Ctrl+Right-Click`, matched per file type
- **Visual Menu Editor (`F9`)** - reorder, add, and configure DSL action rules without touching config files

### Custom Icons

- Assign any image (`.png`, `.jpg`, `.webp`, `.svg`) or symbolic theme icon to files and folders (`F3`)
- Reset to system default (`Ctrl+F3`)
- Associations are re-keyed automatically on rename, move, and drag-and-drop

### Integrated Terminal (`F4`)

- Embedded terminal pane directly below the file grid
- **OSC 7 sync** - `cd` in the terminal automatically navigates the file manager
- Auto-adapts colors to the active Libadwaita theme
- Idle detection via `TIOCGPGRP` before directory respawns

### Properties & Inspection

- File identity: symlink targets, extended attributes, inodes, device IDs
- **Security:** Shannon entropy, SHA-256 hash, ELF architecture/endianness
- **Media:** image dimensions, audio/video duration via `ffprobe`, line/word/TODO counts
- **Git:** commit history for files under version control
- Default app handler - view and reassign MIME type associations

---

## 📦 Archive Browsing (`/archive://`)

Browse archive contents as a virtual filesystem without extraction, with full support for on-the-fly file extraction while navigating, copying and pasting files, and editing zip archives directly.

| Format                                            | Backend                           | Password |
| :------------------------------------------------ | :-------------------------------- | :------: |
| `.zip`                                            | `zip` (deflate, bzip2, zstd, aes) |   Yes    |
| `.7z`                                             | `sevenz-rust` (AES256)            |   Yes    |
| `.rar`                                            | `unar` / `unrar`                  |   Yes    |
| `.tar`, `.tar.gz`, `.tgz`                         | `tar` + `flate2`                  |    No    |
| `.tar.bz2`, `.tbz2`                               | `tar` + `bzip2`                   |    No    |
| `.tar.xz`, `.txz`                                 | `tar` + `xz2`                     |    No    |
| `.tar.zst`, `.tzst`                               | `tar` + `zstd`                    |    No    |
| `.tar.lz4`                                        | `tar` + `lz4_flex`                |    No    |
| `.iso`                                            | ISO 9660 reader                   |    No    |
| `.deb`                                            | ar + tar                          |    No    |
| `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4` (standalone) | Streaming decoders                |    No    |

---

## 🔒 LUKS Encrypted Volumes

- Magic-byte detection of LUKS containers and partition images
- Integrated passphrase dialog, keyfile-based unlock via `udisksctl`
- Eject automatically locks the dm-crypt device and detaches the loop

> Requires `cryptsetup` and `udisks2`.

---

## 🌐 Network Browsing

Integrates with GVFS for remote filesystem access. Connect via `Ctrl+Shift+L`.

| Protocol       | Scheme             |
| :------------- | :----------------- |
| SMB/Samba      | `smb://`           |
| SFTP           | `sftp://`          |
| FTP / FTPS     | `ftp://` `ftps://` |
| WebDAV / HTTPS | `dav://` `davs://` |
| NFS            | `nfs://`           |
| AFP            | `afp://`           |
| MTP            | `mtp://`           |
| Google Drive   | `google-drive://`  |

Active mounts appear in the sidebar with eject controls. Network bookmarks persist in `config.toml`.

> Requires `gvfs` and relevant backend packages (e.g. `gvfs-smb`, `gvfs-sftp`).

---

## ⌨️ Keyboard Shortcuts

| Key                           | Action                                                 |
| :---------------------------- | :----------------------------------------------------- |
| `Backspace` / `Alt + Left`    | Go back in history                                     |
| `Alt + Right`                 | Go forward in history                                  |
| `Ctrl + Home`                 | Navigate to home directory                             |
| `Enter`                       | Open selected file or directory                        |
| `/`                           | Navigate to root directory                             |
| `Ctrl + L`                    | Open location dialog                                   |
| `Ctrl + Shift + L`            | Connect to server                                      |
| `Ctrl + Shift + T`            | Open tag navigator                                     |
| `Insert`                      | Add selection or current folder to list                |
| `Ctrl + Insert`               | Pin selection or current folder to sidebar permanently |
| `Tab`                         | Cycle to the next folder in the list                   |
| `Ctrl + End`                  | Clear the entire list                                  |
| `Ctrl + F`                    | Toggle advanced search sidebar                         |
| `:term` / `:.ext:term`        | Start content search                                   |
| `Esc`                         | Cancel content search                                  |
| `F2`                          | Rename selected item                                   |
| `F3`                          | Toggle folders first in current directory              |
| `F4`                          | Toggle embedded terminal                               |
| `F5`                          | Refresh current directory                              |
| `F6`                          | Toggle header bar                                      |
| `F7`                          | Toggle status bar                                      |
| `F8`                          | Toggle sidebar                                         |
| `Ctrl + Scroll`               | Resize grid items                                      |
| `Ctrl + Middle Click`         | Open folder in new window                              |
| `Ctrl + H`                    | Toggle hidden files                                    |
| `Delete`                      | Move selected items to trash                           |
| `Ctrl + Z`                    | Undo file operation                                    |
| `Ctrl + Shift + Z / Ctrl + Y` | Redo file operation                                    |
| `Ctrl + T`                    | Edit tags for selection                                |
| `Ctrl + Shift + C`            | Copy absolute path of selected items                   |
| `Ctrl + Shift + V`            | Create symbolic link from clipboard paths              |
| `Ctrl + Alt + Shift + V`      | Create hard link from clipboard paths                  |
| `Ctrl + Shift + F7`           | Open memory debug profiler                             |
| `F9`                          | Open context menu editor                               |
| `F10`                         | Open preferences                                       |
| `Ctrl + S`                    | Cycle through sorting modes                            |
| `Ctrl + Shift + S`            | Toggle ascending/descending sort order                 |

## Testing

- **350+ tests** covering core services, UI logic, utilities, and security invariants
- **Fuzzing** via `libFuzzer` on all parsers (archive URIs, glob patterns, search queries, media probes)

```bash
sh tests.sh

# Fuzzing (requires nightly Rust)
cd fuzz/fuzz_targets
python fuzz.py --level default                       # 10k iterations
python fuzz.py --target fuzz_glob --level medium     # 50k
# Levels: default (10k), medium (50k), high (200k), extreme (1M)
```

---

## Installation

### Prerequisites

Flux requires GTK4, Libadwaita, and system media/icon utilities. A **Nerd Font** (or standalone Nerd Font symbols font) must be installed to render the icons in `menu.rs` and the menu editor.

**Ubuntu / Debian**

```bash
sudo apt install libadwaita-1-dev libgtk-4-dev libpango1.0-dev libgraphene-1.0-dev \
  libcairo2-dev libgdk-pixbuf-2.0-dev libpoppler-glib-dev ffmpeg imagemagick icoutils
```

> _Note:_ Debian/Ubuntu repositories do not package standalone Nerd Fonts by default. Install the symbol glyphs with:
>
> ```bash
> mkdir -p ~/.local/share/fonts
> curl -fLo ~/.local/share/fonts/SymbolsNerdFont-Regular.ttf \
>   https://github.com/ryanoasis/nerd-fonts/raw/HEAD/patched-fonts/NerdFontsSymbolsOnly/SymbolsNerdFont-Regular.ttf
> fc-cache -f -v
> ```

**Arch Linux**

```bash
yay -S flux-filemanager-git
```

or for manual install

```bash
sudo pacman -S libadwaita gtk4 glib2 pango graphene cairo gdk-pixbuf2 poppler-glib \
  ffmpeg imagemagick icoutils ttf-nerd-fonts-symbols
```

**Fedora**

```bash
sudo dnf install libadwaita-devel gtk4-devel pango-devel graphene-devel cairo-devel \
  gdk-pixbuf2-devel poppler-glib-devel ffmpeg ImageMagick icoutils
```

> _Note:_ To install the symbol glyphs on Fedora:
>
> ```bash
> sudo dnf copr enable che/nerd-fonts
> sudo dnf install nerd-fonts-SymbolsOnly
> ```

---

### Build & Install

```bash
git clone https://github.com/killown/flux.git
cd flux
cargo build --release
make install
```

---

## Tech Stack

- **Rust 1.75+** - memory safe, zero-cost abstractions
- **Relm4 + GTK4** - native GNOME experience
- **Tokio & Rayon** - non-blocking async I/O and task scheduling
- **Nucleo** - SIMD-accelerated parallel fuzzy matching engine
- **Ignore (ripgrep)** - high-throughput parallel directory walker
- **SQLite (rusqlite)** - tag indexing and persistent state

---

## Contributing

Pull requests are welcome. For major changes please open an issue first. Run the test suite before submitting.

## License

[GPLv3](https://github.com/killown/flux/blob/main/LICENSE)
