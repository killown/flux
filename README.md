# 🌊 Flux

![Rust](https://img.shields.io/badge/Rust-1.75%2B-black?logo=rust)
![GTK4](https://img.shields.io/badge/GTK-4-green?logo=gnome)
![License](https://img.shields.io/badge/License-GPLv3-blue.svg)

**Flux** is a minimalist, high-performance file manager for Linux built for those who love the clean look of GNOME but want a tool that stays out of their way.

<img width="1940" height="1056" alt="flux-screenshot" src="https://github.com/user-attachments/assets/bdcdead3-883b-4a29-9148-9b16278f9f5f" />

## Why Flux?

```bash
#CPU: AMD Ryzen 5 5600X (6 Cores, 12 Threads @ 3.7GHz Base / 4.6GHz Boost)
#Methodology: High-precision time.perf_counter() differential, IPC event stream.

~ ❯❯❯ python measure_app_speed.py flux-fm
Startup Time: 121.52 ms
~ ❯❯❯ python measure_app_speed.py thunar
Startup Time: 250.01 ms   # warm: 189.40 ms
~ ❯❯❯ python measure_app_speed.py nautilus
Startup Time: 742.84 ms   # warm: 449.54 ms
```

- **Zero Clutter:** A smart dynamic header shows exactly what you need.
- **Asynchronous Heart:** Throttled async pipeline for thumbnails and directory loads, never freezes.
- **Shortcut First:** Designed for keyboard-driven power users.

---

## Features

### Navigation

- Dual view modes: responsive grid cards and compact list view
- Breadcrumb navigation supporting local paths, `/archive://` virtual roots, and network URIs
- Forward/backward history (`Alt+Left`, `Alt+Right`, `Backspace`, mouse side buttons)
- Automatic UI sync on filesystem changes via GIO directory monitors
- Configurable sorting by Name, Date, Size, or Type with Folders First toggle

### Search & Filtering

**Instant filename filter** - type to filter the current directory live.

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

- `*.png`, `*.rs` - extension globs
- `image/*`, `video/*`, `audio/*` - built-in category shorthands
- `application/zip`, `image/png` - any system MIME type (resolved via `/usr/share/mime/globs`)

**Advanced Search Dialog (`F12`)** - modal combining name, content, extension, date range, size constraints, and recursion depth toggles.

**Tag search** - FreeDesktop `user.xdg.tags` xattr system with SQLite indexing. Use `#tagname` or `:tag:name` to filter globally, with a dedicated tag navigator.

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

Browse archive contents as a virtual filesystem without extraction.

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

| Key                | Action                                 |
| :----------------- | :------------------------------------- |
| `F1`               | Keyboard shortcut reference            |
| `F2` / `Ctrl+I`    | Inline rename                          |
| `F3`               | Set custom icon                        |
| `Ctrl+F3`          | Reset custom icon                      |
| `F4`               | Toggle integrated terminal             |
| `F9`               | Visual menu editor                     |
| `F12`              | Advanced search dialog                 |
| `Ctrl+F`           | Search / filter bar                    |
| `Ctrl+H`           | Toggle hidden files                    |
| `Ctrl+L`           | Go to location                         |
| `Ctrl+Shift+L`     | Connect to server                      |
| `Ctrl+S`           | Cycle sort criteria                    |
| `Ctrl+Shift+S`     | Toggle sort order                      |
| `Shift+S`          | Toggle folders first                   |
| `Ctrl+A`           | Select all                             |
| `Ctrl+C / X / V`   | Copy / Cut / Paste                     |
| `Delete`           | Move to trash                          |
| `Ctrl+Z`           | Undo                                   |
| `Ctrl+Shift+Z`     | Redo                                   |
| `Alt+Left/Right`   | Back / Forward                         |
| `Insert`           | Pin to quick list                      |
| `Tab`              | Next quick list entry                  |
| `Ctrl+PageUp/Down` | Previous / next quick list entry       |
| `Ctrl+End`         | Clear quick list                       |
| `Ctrl+Insert`      | Pin current dir to sidebar permanently |

---

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

## Extra Themes

```bash
git clone https://github.com/killown/flux-themes.git
mkdir -p ~/.local/share/flux/themes
cp flux-themes/themes/* ~/.local/share/flux/themes/
```

---

## Installation

**Ubuntu / Debian**

```bash
sudo apt install libadwaita-1-dev libgtk-4-dev libpango1.0-dev libgraphene-1.0-dev \
  libcairo2-dev libgdk-pixbuf-2.0-dev libpoppler-glib-dev ffmpeg imagemagick
```

**Arch Linux**

```bash
sudo pacman -S libadwaita gtk4 glib2 pango graphene cairo gdk-pixbuf2 poppler-glib ffmpeg imagemagick
```

**Fedora**

```bash
sudo dnf install libadwaita-devel gtk4-devel pango-devel graphene-devel cairo-devel \
  gdk-pixbuf2-devel poppler-glib-devel ffmpeg ImageMagick
```

```bash
git clone https://github.com/killown/flux.git
cd flux
cargo build --release
```

---

## Tech Stack

- **Rust 1.75+** - memory safe, zero-cost abstractions
- **Relm4 + GTK4** - native GNOME experience
- **Tokio** - non-blocking async I/O
- **SQLite (rusqlite)** - tag indexing and persistent state

---

## Contributing

Pull requests are welcome. For major changes please open an issue first. Run the test suite before submitting.

## License

[GPLv3](https://github.com/killown/flux/blob/main/LICENSE)
