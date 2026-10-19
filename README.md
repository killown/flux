# Flux

![Rust](https://img.shields.io/badge/Rust-stable-black?logo=rust)
![GTK4](https://img.shields.io/badge/GTK-4-green?logo=gnome)
![License](https://img.shields.io/badge/License-GPLv3-blue.svg)

Flux is a GTK4/Libadwaita file manager for Linux, written in Rust. I wanted something that opens instantly, looks at home on GNOME and Wayland, and can be driven from the keyboard. It does that, and it does it faster than the other GUI file managers I tried.

<p align="center">
  <a href="https://youtu.be/Hv7yWl2wjSk">
    <img src="https://github.com/killown/flux/raw/main/screenshots/flux.png" alt="Flux, a GTK4 Libadwaita file manager for Linux, with the preview panel and embedded terminal" width="100%">
  </a>
</p>

> The icons shown in the screenshot are from [WhiteSur Icon Theme](https://github.com/vinceliuice/WhiteSur-icon-theme) (GPL-3.0).

## Startup time

Cold start on a Ryzen 5 5600X, mean of 10 runs:

| File manager | Startup |
| :----------- | ------: |
| Flux         |  105 ms |
| PCManFM      |  134 ms |
| SpaceFM      |  169 ms |
| Thunar       |  290 ms |
| Nautilus     |  743 ms |

## Features

- **Archives behave like folders.** Open zip, 7z, rar, tar, iso and deb files like any directory, with thumbnails and previews of what's inside. Nothing gets extracted.
- **Search that's already there.** Type to filter the folder, press `Ctrl+F` for a search panel on the right, or start with `:` to search inside files. There is no index to build and no background service.
- **Your servers and drives.** SMB, SFTP, FTP, WebDAV, NFS, MTP and Google Drive, plus encrypted LUKS volumes.
- **A terminal that follows you.** Press `F4`, `cd` somewhere, and the file view goes with you.
- **Tags.** Label files and find them again with `#tag` or `Ctrl+Shift+T`. Tags use the standard `user.xdg.tags` file attribute, so they stay with the file.
- **A preview panel that keeps up.** Images, video, audio and PDFs show as you move through files. `F11` goes wide, and videos even play right in the grid. Code (with syntax highlighting), Markdown, CSV tables, Office documents and fonts preview too. Previews only show what's inside a file, editing is left to your preferred app.

<p align="center">
  <img src="https://github.com/killown/flux/raw/main/screenshots/preview.png" alt="Flux preview panel showing a file preview next to the file grid" width="100%">
</p>

- **Icons and covers.** Change the icon for a file type, a single file or a folder with any image. Use it to give videos, games and music their own cover art, and set whole batches from the command line.

<p align="center">
  <img src="https://github.com/killown/flux/raw/main/screenshots/covers.png" alt="Flux grid view with custom cover art set as folder icons for a game library" width="100%">
</p>

## Install

Flux needs GTK 4 and libadwaita 1.5 or newer, so Debian 12 and Ubuntu 22.04 are too old.

Arch (AUR):

```bash
yay -S flux-filemanager-git
```

From source, install the dependencies first.

Debian / Ubuntu:

```bash
sudo apt install build-essential pkg-config git gettext desktop-file-utils libadwaita-1-dev libgtk-4-dev libpango1.0-dev libgraphene-1.0-dev \
  libcairo2-dev libgdk-pixbuf-2.0-dev libpoppler-glib-dev ffmpeg imagemagick icoutils p7zip-full
```

Fedora:

```bash
sudo dnf install gcc make pkgconf-pkg-config git gettext desktop-file-utils libadwaita-devel gtk4-devel pango-devel graphene-devel cairo-devel \
  gdk-pixbuf2-devel poppler-glib-devel ffmpeg ImageMagick icoutils p7zip
```

Arch:

```bash
sudo pacman -S base-devel git gettext desktop-file-utils libadwaita gtk4 glib2 pango graphene cairo gdk-pixbuf2 poppler-glib \
  ffmpeg imagemagick icoutils ttf-nerd-fonts-symbols p7zip
```

Then build it:

```bash
git clone https://github.com/killown/flux.git
cd flux
cargo build --release
make install
```

`make install` installs for the current user into `~/.local`. The binary is `~/.local/bin/flux-fm`, so make sure that folder is in your `PATH`. It also copies the default menus and helper scripts and downloads the community themes (this needs `git` and a connection, and is skipped if you're offline).

The default right-click menu is a starting point. Some entries call tools such as `alacritty`, `wl-clipboard`, `yt-dlp` or Python with PyGObject, so edit `menu.rs` to match what you have installed.

The menu editor uses Nerd Font icons. Arch gets the font from the package above. On Debian, Ubuntu and Fedora, download [SymbolsNerdFont-Regular.ttf](https://github.com/ryanoasis/nerd-fonts/raw/HEAD/patched-fonts/NerdFontsSymbolsOnly/SymbolsNerdFont-Regular.ttf) into `~/.local/share/fonts` and run `fc-cache -f`.

Network browsing needs `gvfs` plus a backend such as `gvfs-smb` or `gvfs-sftp`. LUKS needs `cryptsetup` and `udisks2`.

## Shortcuts

| Key                      | Action                    |
| :----------------------- | :------------------------ |
| `Ctrl+L`                 | Go to location            |
| `Ctrl+F`                 | Search panel (right side) |
| `Alt+Left` / `Alt+Right` | Back / forward            |
| `F2`                     | Rename                    |
| `F4`                     | Terminal                  |
| `F5`                     | Refresh                   |
| `F8`                     | Sidebar                   |
| `F9`                     | Context menu editor       |
| `F10`                    | Preferences               |
| `F11`                    | Wide preview              |
| `Ctrl+H`                 | Hidden files              |
| `Ctrl+Z` / `Ctrl+Y`      | Undo / redo               |
| `Ctrl+T`                 | Edit tags                 |
| `Ctrl+Middle Click`      | Open folder in new window |

`F1` shows the rest.

## Customization

Flux is built to be changed:

- Settings live in `~/.config/flux/config.toml`, and most of them are in Preferences (`F10`).
- Style the whole interface with CSS in `~/.config/flux/style.css`. It loads on top of the theme and reloads when you save.
- Install themes into `~/.local/share/flux/themes`. Community themes come from [flux-themes](https://github.com/killown/flux-themes).
- Use any image as the window or sidebar background from the right-click menu.
- Rearrange the sidebar: drag and drop to move items and create new sections.
- Move the window controls to either side.
- Hide the sidebar, header bar and status bar (`F8`, `F6`, `F7`), or start with `--no-sidebar`, `--no-header` or `--no-statusbar`.
- Hide file extensions.
- Set a custom icon for a single file or folder, or for every file with a given extension.
- Set or reset icons in bulk from the command line, with one `TARGET<TAB>IMAGE` pair per line:

```bash
printf '%s\t%s\n' ~/Videos ~/icons/video.svg ~/Music ~/icons/music.svg | flux-fm --set-icons-stdin
```

- Edit the right-click menus by hand in `~/.config/flux/menu.rs`, or use the editor on `F9`. A second menu on `Ctrl+Right-Click` changes with the file type, so the main menu can stay short:

```rust
"󰉋   Inspect Directory" => "directory", "builtin::inspect_dir"
"󰋜   Open With…"        => "all", "builtin::open_with_dialog"
"󰆴   Move to Trash"     => "all", "builtin::delete"
```

## Under the hood

Rust, Relm4 and GTK4, Tokio and Rayon, Nucleo for fuzzy matching, the `ignore` crate for directory walking, and SQLite for tags. There are 600+ tests (`sh tests.sh`), and the parsers are fuzzed with libFuzzer.

## Contributing

Pull requests are welcome. If the change is big, open an issue first. Please run the tests before sending it.

## License

[GPLv3](https://github.com/killown/flux/blob/main/LICENSE)
