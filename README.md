# 🌊 Flux

![Rust](https://img.shields.io/badge/Rust-1.75%2B-black?logo=rust)
![GTK4](https://img.shields.io/badge/GTK-4-green?logo=gnome)
![License](https://img.shields.io/badge/License-GPLv3-blue.svg)

**Flux** is a minimalist, high-performance file manager for Linux. It’s built for those who love the clean look of GNOME but want a tool that stays out of their way.
<img width="1960" height="1081" alt=" from 2026-08-25 16-38-42" src="https://github.com/user-attachments/assets/de75f947-4d6c-410f-8fbd-54364540cc91" />

## Why Flux?

To keep benchmarks completely transparent, here is the real **Time to First Frame** (measured via IPC event-stream watching) showing both **cold-start** (first run) and **warm-start** (second run, leveraging OS page cache) times:

```bash
#CPU: AMD Ryzen 5 5600X (6 Cores, 12 Threads @ 3.7GHz Base / 4.6GHz Boost)
#Methodology: High-precision time.perf_counter() differential, IPC event stream.

# 1. Flux-FM (Consistent performance, no cold-start overhead)
~ ❯❯❯ python measure_app_speed.py flux-fm
Startup Time: 229.05 ms
~ ❯❯❯ python measure_app_speed.py flux-fm
Startup Time: 226.04 ms

# 2. Thunar (Relies heavily on page cache to feel fast)
~ ❯❯❯ python measure_app_speed.py thunar
Startup Time: 250.01 ms
~ ❯❯❯ python measure_app_speed.py thunar
Startup Time: 196.79 ms

# 3. Nautilus (Heavy startup bottlenecks)
~ ❯❯❯ python measure_app_speed.py nautilus
Startup Time: 742.84 ms
~ ❯❯❯ python measure_app_speed.py nautilus
Startup Time: 449.54 ms
```

Most file managers try to do everything. Flux tries to do _one thing_ perfectly: letting you browse your data at light speed without the visual noise.

- **Zero Clutter:** We replaced bulky buttons with a smart, dynamic header that shows you exactly what you need to know.
- **Asynchronous Heart:** Got a folder with 5,000 high-res wallpapers? Flux won't sweat. It uses a throttled async pipeline to load thumbnails without ever freezing the window.
- **Shortcut First:** Flux is designed for power users who prefer the keyboard over hunting for tiny icons.

## Testing

Flux is built with a comprehensive test suite to ensure stability and correctness.

- **Unit & integration tests:** Over **350+ tests** covering core services, UI logic, utilities, and security invariants.
- **Fuzzing:** Continuous fuzzing (via `libFuzzer`) is used to stress‑test all parsers, including archive URIs, glob patterns, search queries, and media probes. This has already uncovered real edge‑case bugs that would otherwise have crashed the application.

### Running tests

```bash
# Run the full test suite
sh tests.sh

# Run fuzzing (optional, requires nightly Rust)
cd fuzz/fuzz_targets
python fuzz.py --level default

# Run a specific target with more iterations
python fuzz.py --target fuzz_glob --level medium

# Available levels: default (10k), medium (50k), high (200k), extreme (1M)
```

## 📦 Archive & Compressed File Browsing

Flux features an integrated virtual filesystem layer (`/archive://`) that allows you to open, browse, and copy contents out of compressed archives just like normal directories.

### Supported Formats & Capabilities

| Extension                                         | Backend Engine                    | Password Support |
| :------------------------------------------------ | :-------------------------------- | :--------------: |
| `.zip`                                            | `zip` (deflate, bzip2, zstd, aes) |       Yes        |
| `.7z`                                             | `sevenz-rust` (AES256)            |       Yes        |
| `.rar`                                            | External CLI (`unar` / `unrar`)   |       Yes        |
| `.tar`, `.tar.gz`, `.tgz`                         | `tar` + `flate2`                  |        No        |
| `.tar.bz2`, `.tbz2`                               | `tar` + `bzip2`                   |        No        |
| `.tar.xz`, `.txz`                                 | `tar` + `xz2`                     |        No        |
| `.tar.zst`, `.tzst`                               | `tar` + `zstd`                    |        No        |
| `.tar.lz4`                                        | `tar` + `lz4_flex`                |        No        |
| `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4` (standalone) | Streaming decoders                |        No        |

## 🔒 Encrypted Volumes (LUKS)

Flux provides built-in support for unlocking and mounting LUKS-encrypted drives and container files.

- **Seamless Prompting:** Attempting to open a LUKS-encrypted partition or image triggers an integrated passphrase dialog.
- **Auto-Mounting:** Once authenticated, the decrypted volume is mounted automatically and appears under your sidebar locations.
- **Eject & Lock:** Unmounting a LUKS volume automatically locks the encrypted container.

> **Prerequisite:** Ensure `cryptsetup` and `udisks2` are installed on your system.

## 🌐 Network Browsing

Flux integrates with **GVFS** (GNOME Virtual File System) to provide seamless access to remote servers and network shares. You can browse SMB (Windows shares), SFTP, FTP, WebDAV, NFS, and more directly from the file manager.

- **Connect to Server** - Press `Ctrl+Shift+L` or use the hamburger menu to open a connection dialog. Enter the protocol, server address, port, and optional path.
- **Bookmarks** - Network locations can be added to the sidebar for quick access. They are stored in `config.toml` under `network_bookmarks`.
- **Active mounts** - Connected servers appear in a dynamic "Connected Servers" section in the sidebar, with an eject button to unmount them.

### Supported Protocols

| Protocol       | Scheme            | Description                                    |
| :------------- | :---------------- | :--------------------------------------------- |
| SMB/Samba      | `smb://`          | Windows file sharing                           |
| SFTP           | `sftp://`         | SSH File Transfer Protocol                     |
| FTP            | `ftp://`          | File Transfer Protocol                         |
| FTPS           | `ftps://`         | FTP over TLS                                   |
| WebDAV         | `dav://`          | Web-based Distributed Authoring and Versioning |
| WebDAV (HTTPS) | `davs://`         | WebDAV over TLS                                |
| NFS            | `nfs://`          | Network File System                            |
| AFP            | `afp://`          | Apple Filing Protocol                          |
| MTP            | `mtp://`          | Media Transfer Protocol (devices)              |
| Google Drive   | `google-drive://` | Google Drive (via GVFS)                        |

> **Prerequisite:** Install `gvfs` and the relevant backend packages (e.g., `gvfs-smb`, `gvfs-sftp`) from your distribution.

## 🎨 Custom Icon Overrides

Flux allows you to assign custom visual icons or artwork to individual files and folders.

- **Set Custom Image/Icon:** Right-click any file or folder and select **Set Custom Icon** (or press `F3` for folders) to pick any custom image (`.png`, `.jpg`, `.webp`, `.svg`).
- **Reset to Default:** Right-click the item and select **Reset Custom Icon** (or press `Ctrl+F3`) to restore the system default icon.
- **Persistence Across Moves:** Custom icon associations are automatically re-keyed and preserved when renaming, cutting and pasting, or drag-and-drop moving items.

## ⌨️ Keyboard Shortcut & Input Hub Reference

### 6.1 Unified Global Bindings

The following table details the core hotkey mappings defined across `src/ui/inputs.rs`, `src/utils/helpers.rs`, and the shortcut controller stack:

| Key Combination       | Message Reference / Operation Target | Functional System Description                                                           |
| :-------------------- | :----------------------------------- | :-------------------------------------------------------------------------------------- |
| `F1`                  | `AppMsg::ShowHelp`                   | Opens the graphical keyboard shortcut reference window.                                 |
| `F2`                  | `AppMsg::TriggerRenameSelection`     | Puts the selected item into inline name editing mode.                                   |
| `F3`                  | `AppMsg::TriggerIconPicker`          | Opens a custom icon picker dialog for the selected directory.                           |
| `Ctrl + F3`           | `AppMsg::TriggerResetIcon`           | Resets the selected directory's icon back to the system default.                        |
| `F4`                  | `AppMsg::ToggleTerminal`             | Toggles the visibility of the embedded virtual terminal panel.                          |
| `F5`                  | `AppMsg::Refresh`                    | Forces a full reload of the current directory from disk.                                |
| `F8`                  | `AppMsg::ToggleSidebar`              | Toggles the left navigation sidebar tray.                                               |
| `F9`                  | `app.open-menu-editor`               | Spawns the runtime context menu configuration editor window.                            |
| `F10`                 | `app.open-settings`                  | Launches the central preference settings interface panel.                               |
| `Return` / `KP_Enter` | `AppMsg::Activate`                   | Opens selected files or navigates into selected folders.                                |
| `Backspace`           | `AppMsg::GoBack`                     | Navigates back one step in the directory history log.                                   |
| `Alt + Left`          | `AppMsg::GoBack`                     | Navigates back one step in the directory history log.                                   |
| `Alt + Right`         | `AppMsg::GoForward`                  | Moves forward one step in the directory history stack.                                  |
| `/`                   | `AppMsg::Navigate("/")`              | Jumps directly to the filesystem root.                                                  |
| `Ctrl + N`            | `app.new-window`                     | Opens a new Flux window instance.                                                       |
| `Ctrl + C`            | `AppMsg::Copy`                       | Copies the selected items to the clipboard.                                             |
| `Ctrl + X`            | `AppMsg::Cut`                        | Cuts the selected items to the clipboard.                                               |
| `Ctrl + V`            | `AppMsg::Paste`                      | Pastes clipboard items into the current directory.                                      |
| `Delete`              | `AppMsg::Delete`                     | Moves selected items to trash.                                                          |
| `Ctrl + F`            | `AppMsg::SwitchHeader("search")`     | Focuses the search entry and switches the header to filename filtering mode.            |
| `Ctrl + H`            | `AppMsg::ToggleHidden`               | Toggles the visibility of hidden files and folders.                                     |
| `Ctrl + I`            | `AppMsg::TriggerRenameSelection`     | Opens inline rename for the selected item (same as F2).                                 |
| `Ctrl + S`            | `AppMsg::CycleSort`                  | Rotates through the available sorting criteria.                                         |
| `Ctrl + Shift + S`    | `AppMsg::ToggleSortOrder`            | Toggles between ascending and descending sort order.                                    |
| `Shift + S`           | `AppMsg::CycleFolderPriority`        | Toggles whether folders are grouped above files when sorting.                           |
| `Ctrl + L`            | `AppMsg::PromptLocationDialog`       | Opens the "Go to Location" dialog to type a path or network URI.                        |
| `Ctrl + Shift + L`    | `app.connect-to-server`              | Opens the "Connect to Server" dialog for mounting remote shares (SMB, SFTP, FTP, etc.). |
| `Insert`              | `AppMsg::AddExclusive(None)`         | Adds the selected path to the temporary quick list cache.                               |
| `Ctrl + Insert`       | `AppMsg::AddToSidebarPermanent`      | Pins the current directory to the configuration bookmarks permanently.                  |
| `Tab`                 | `AppMsg::NextExclusive`              | Cycles forward to the next folder in the temporary quick list.                          |
| `Ctrl + Page Down`    | `AppMsg::NextExclusive`              | Cycles forward to the next folder in the temporary quick list.                          |
| `Ctrl + Page Up`      | `AppMsg::PrevExclusive`              | Cycles backward to the previous folder in the temporary quick list.                     |
| `Ctrl + End`          | `AppMsg::ClearExclusive`             | Clears all entries from the temporary quick list cache.                                 |

## 🔍 Search & Filtering

Flux features a powerful dual-mode search and filtering engine accessible via `Ctrl + F`.

### 1. Filename & Size Filtering (Quick Bar)

As you type in the search bar, it filters filenames instantly. You can also filter files by size using comparison operators:

| Pattern      | Matches                                    |
| :----------- | :----------------------------------------- |
| `>10MB`      | Files larger than 10 MB                    |
| `<500KB`     | Files smaller than 500 KB                  |
| `10MB..50MB` | Files between 10 MB and 50 MB              |
| `>1GB video` | Combines size filtering with name matching |

_Supported units:_ `B`, `K` / `KB`, `M` / `MB`, `G` / `GB`, and `T` / `TB`.

### 2. Deep Content Search (`:term`)

Prefix your query with a colon (`:`) to recursively search **inside** text-based file contents across the directory tree rather than just matching filenames.

- **Search all files:** `:term` (e.g., `:fn main`)
- **Filter by extension:** `:.ext:term` (e.g., `:.rs:search_term` or `:.rs,py:tokio`)
- **Requirements:** The search term must be at least 3 characters long. Press `Esc` or click the stop button to cancel an active search.

## Configuration & Customization

Everything is managed via `~/.config/flux/config.toml`. Want to add a custom folder to your sidebar or change the default sorting? Just edit the text file. It's that simple.

> **Custom Actions:** You can define your own right-click commands. Add "Open in VS Code" or "Optimize Image" by linking a simple shell command in your config.

## Extra Themes

```bash
git clone https://github.com/killown/flux-themes.git
cd flux-themes
mkdir -p ~/.local/share/flux/themes
cp themes/* ~/.local/share/flux/themes/
```

## Tech Stack

- **Language:** Rust 1.75+ (Memory safe and blazing fast)
- **UI Framework:** Relm4 & GTK4 (Native GNOME experience)
- **Runtime:** Tokio & Futures (Non-blocking I/O)

### flux - build & runtime requirements

#### system libraries (pkg-config)

- **libadwaita-1** → modern widgets and adaptive layout capabilities.
- **gtk4** → the primary toolkit for window management and rendering.
- **gdk-pixbuf-2.0** → image loading and scaling for file icons.
- **cairo-gobject** → vector graphics rendering for custom ui elements.
- **graphene-1.0** → hardware-accelerated 2d/3d transformations.
- **gio-2.0** → virtual file system operations and directory monitoring.
- **glib-2.0** → core event loop and data structure management.
- **gobject-2.0** → the type system required for rust-to-c interoperability.
- **pango** → font handling and phonetic text layout.
- **gvfs** → virtual filesystem backends for network browsing (SMB, SFTP, FTP, etc.).

#### runtime dependencies

- **ffmpeg** → video decoding and frame capture.
- **ffprobe** → extraction of technical media metadata.
- **magick** → image conversion and processing (imagemagick).

---

## Installation

### Prerequisites

Flux requires the following system libraries and runtime tools:

| Category    | Dependencies                                                                               |
| :---------- | :----------------------------------------------------------------------------------------- |
| **System**  | `libadwaita`, `gtk4`, `glib2`, `pango`, `graphene`, `cairo`, `gdk-pixbuf2`, `poppler-glib` |
| **Runtime** | `ffmpeg`, `ffprobe`, `ImageMagick`                                                         |

**Ubuntu / Debian / Linux Mint**

```bash
sudo apt update
sudo apt install libadwaita-1-dev libgtk-4-dev libpango1.0-dev libgraphene-1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev libpoppler-glib-dev ffmpeg imagemagick
```

**Arch Linux**

```bash
sudo pacman -S libadwaita gtk4 glib2 pango graphene cairo gdk-pixbuf2 poppler-glib ffmpeg imagemagick
```

**Fedora**

```bash
sudo dnf install libadwaita-devel gtk4-devel pango-devel graphene-devel cairo-devel gdk-pixbuf2-devel poppler-glib-devel ffmpeg ImageMagick
```

### Build

```bash
git clone https://github.com/killown/flux.git
cd flux
cargo build --release
```

## Contributing

Pull requests are welcome! For major changes, please open an issue first to discuss what you would like to change.
Make sure to run the tests before submitting your PR.

## Support

If you encounter any bugs or have feature requests, please open an issue on the [GitHub Issue Tracker](https://github.com/killown/flux/issues).

## License

Flux is free and open-source software licensed under the **[GPLv3](https://github.com/killown/flux/blob/main/LICENSE)**.
