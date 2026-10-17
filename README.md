# Flux

![Rust](https://img.shields.io/badge/Rust-1.75%2B-black?logo=rust)
![GTK4](https://img.shields.io/badge/GTK-4-green?logo=gnome)
![License](https://img.shields.io/badge/License-GPLv3-blue.svg)

Flux is a GTK4/Libadwaita file manager for Linux, written in Rust. I wanted something that opens instantly, looks at home on GNOME and Wayland, and can be driven from the keyboard. It does that, and it does it faster than the other GUI file managers I tried.

<p align="center">
  <a href="https://www.youtube.com/watch?v=CUsf3Xyxw-k">
    <img src="https://github.com/killown/flux/raw/main/screenshots/flux.png" alt="Flux, a GTK4 Libadwaita file manager for Linux, with the preview panel and embedded terminal" width="100%">
  </a>
</p>

## Startup time

Cold start on a Ryzen 5 5600X, mean of 10 runs:

| File manager | Startup |
| :----------- | ------: |
| Flux         |  105 ms |
| PCManFM      |  134 ms |
| SpaceFM      |  169 ms |
| Thunar       |  290 ms |
| Nautilus     |  743 ms |

## What it does

- Filter the current folder as you type (fuzzy, forgives typos). Size filters work too: `>10MB`, `<500KB`, `10MB..50MB`. Globs and MIME types work: `*.png`, `image/*`.
- Search inside files by starting with a colon: `:term`, or `:.rs,py:term` to limit it to some extensions. `Ctrl+F` opens a search panel with name, content, date, size and depth options.
- Browse zip, 7z, rar, tar.\*, iso and deb files like folders, without extracting. Password-protected zip/7z/rar are supported, and you can edit zips in place.
- Open SMB, SFTP, FTP, WebDAV, NFS, MTP and Google Drive locations through GVFS (`Ctrl+Shift+L`).
- Unlock and eject LUKS volumes.
- Preview images, video, audio, PDFs and archives in a side panel that follows your selection.
- Built-in terminal on `F4`. When you `cd` in it, the file view follows.
- Tag files and find them again with `#tag` or `Ctrl+Shift+T`.
- Copy and move in a background queue with speed and ETA. Conflicts are resolved in a dialog, undo/redo works for renames, moves and trash, and system paths are protected from deletion.
- Edit the right-click menus in a visual editor (`F9`), set custom icons for files, folders and extensions, and add your own CSS.
- The properties window shows SHA-256, entropy, ELF info, media duration and size, and git history.

## Install

Arch (AUR):

```bash
yay -S flux-filemanager-git
```

From source, install the dependencies first.

Debian / Ubuntu:

```bash
sudo apt install libadwaita-1-dev libgtk-4-dev libpango1.0-dev libgraphene-1.0-dev \
  libcairo2-dev libgdk-pixbuf-2.0-dev libpoppler-glib-dev ffmpeg imagemagick icoutils p7zip-full
```

Fedora:

```bash
sudo dnf install libadwaita-devel gtk4-devel pango-devel graphene-devel cairo-devel \
  gdk-pixbuf2-devel poppler-glib-devel ffmpeg ImageMagick icoutils p7zip
```

Arch:

```bash
sudo pacman -S libadwaita gtk4 glib2 pango graphene cairo gdk-pixbuf2 poppler-glib \
  ffmpeg imagemagick icoutils ttf-nerd-fonts-symbols p7zip
```

Then build it:

```bash
git clone https://github.com/killown/flux.git
cd flux
cargo build --release
make install
```

The menu editor uses Nerd Font icons. Arch gets the font from the package above. On Debian, Ubuntu and Fedora, download [SymbolsNerdFont-Regular.ttf](https://github.com/ryanoasis/nerd-fonts/raw/HEAD/patched-fonts/NerdFontsSymbolsOnly/SymbolsNerdFont-Regular.ttf) into `~/.local/share/fonts` and run `fc-cache -f`.

Network browsing needs `gvfs` plus a backend such as `gvfs-smb` or `gvfs-sftp`. LUKS needs `cryptsetup` and `udisks2`.

## Shortcuts

| Key                      | Action                    |
| :----------------------- | :------------------------ |
| `Ctrl+L`                 | Go to location            |
| `Ctrl+F`                 | Advanced search           |
| `Alt+Left` / `Alt+Right` | Back / forward            |
| `F2`                     | Rename                    |
| `F4`                     | Terminal                  |
| `F5`                     | Refresh                   |
| `F8`                     | Sidebar                   |
| `F9`                     | Context menu editor       |
| `F10`                    | Preferences               |
| `Ctrl+H`                 | Hidden files              |
| `Ctrl+Z` / `Ctrl+Y`      | Undo / redo               |
| `Ctrl+T`                 | Edit tags                 |
| `Ctrl+Middle Click`      | Open folder in new window |

`F1` shows the rest.

## Customizing menus

Right-click menus come from `~/.config/flux/menu.rs`. You can edit it by hand or use the editor on `F9`:

```rust
"󰉋   Inspect Directory" => "directory", "builtin::inspect_dir"
"󰋜   Open With…"        => "all", "builtin::open_with_dialog"
"󰆴   Move to Trash"     => "all", "builtin::delete"
```

Styling goes in `~/.config/flux/style.css`. It loads on top of the theme and reloads when you save.

## Under the hood

Rust, Relm4 and GTK4, Tokio and Rayon, Nucleo for fuzzy matching, the `ignore` crate for directory walking, and SQLite for tags. There are 600+ tests (`sh tests.sh`), and the parsers are fuzzed with libFuzzer.

## Contributing

Pull requests are welcome. If the change is big, open an issue first. Please run the tests before sending it.

## License

[GPLv3](https://github.com/killown/flux/blob/main/LICENSE)
