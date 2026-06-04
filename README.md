

# 🌊 Flux

![Rust](https://img.shields.io/badge/Rust-1.75%2B-black?logo=rust)
![GTK4](https://img.shields.io/badge/GTK-4-green?logo=gnome)
![License](https://img.shields.io/badge/License-GPLv3-blue.svg)

**Flux** is a minimalist, high-performance file manager for Linux. It’s built for those who love the clean look of GNOME but want a tool that stays out of their way.

<img width="1244" height="879" alt="focused_view" src="https://github.com/user-attachments/assets/479ed40e-0d26-48a3-b087-7e5f0dca8b6c" />

## Why Flux?

Most file managers try to do everything. Flux tries to do _one thing_ perfectly: letting you browse your data at light speed without the visual noise.

- **Zero Clutter:** We replaced bulky buttons with a smart, dynamic header that shows you exactly what you need to know.
- **Asynchronous Heart:** Got a folder with 5,000 high-res wallpapers? Flux won't sweat. It uses a throttled async pipeline to load thumbnails without ever freezing the window.
- **Shortcut First:** Flux is designed for power users who prefer the keyboard over hunting for tiny icons.

## Power User Shortcuts

Flux is built around the "Config-Driven" philosophy. You control the logic, we provide the speed.

| Action | Shortcut |
| :--- | :--- |
| **Show Help / Shortcuts** | `F1` |
| **Cycle Sort Mode** (Name → Date → Size) | `Ctrl + S` |
| **Toggle Hidden Files** | `Ctrl + H` |
| **Zoom Icons** (Smooth Scaling) | `Ctrl + Mouse Wheel` |
| **Search Files** | `Ctrl + F` |
| **Open Preferences** | `F10` |
| **Open Menu Editor** | `F9` |
| **Refresh Directory** | `F5` |
| **Rename Item** | `F2` |
| **Move to Trash** | `Delete` |
| **Go Back / Forward** | `Backspace` / `Alt + Right` |
| **Navigate to Root** | `/` |

### Quick List (Exclusive List)
- `Insert`: Add selection or current folder to list.
- `Ctrl + Insert`: Pin selection/folder to sidebar permanently.
- `Tab`: Cycle to the next folder in the list.
- `Ctrl + End`: Clear the entire list.

## Configuration & Customization

Everything is managed via `~/.config/flux/config.toml`. Want to add a custom folder to your sidebar or change the default sorting? Just edit the text file. It's that simple.

> **Custom Actions:** You can define your own right-click commands. Add "Open in VS Code" or "Optimize Image" by linking a simple shell command in your config.

## Extra Themes

    git clone https://github.com/killown/flux-themes.git
    cd flux-themes
    cp themes/* ~/.local/share/flux/themes

## Tech Stack

- **Language:** Rust 1.75+ (Memory safe and blazing fast)
- **UI Framework:** Relm4 & GTK4 (Native GNOME experience)
- **Runtime:** Tokio & Futures (Non-blocking I/O)
### flux - build & runtime requirements

#### system libraries (pkg-config)
*   **libadwaita-1** → modern widgets and adaptive layout capabilities.
*   **gtk4** → the primary toolkit for window management and rendering.
*   **gdk-pixbuf-2.0** → image loading and scaling for file icons.
*   **cairo-gobject** → vector graphics rendering for custom ui elements.
*   **graphene-1.0** → hardware-accelerated 2d/3d transformations.
*   **gio-2.0** → virtual file system operations and directory monitoring.
*   **glib-2.0** → core event loop and data structure management.
*   **gobject-2.0** → the type system required for rust-to-c interoperability.
*   **pango** → font handling and phonetic text layout.

#### runtime dependencies
*   **ffmpeg** → video decoding and frame capture.
*   **ffprobe** → extraction of technical media metadata.
*   **magick** → image conversion and processing (imagemagick).

---

## Installation

### Prerequisites
Flux requires the following system libraries and runtime tools:

| Category | Dependencies |
| :--- | :--- |
| **System** | `libadwaita`, `gtk4`, `glib2`, `pango`, `graphene`, `cairo`, `gdk-pixbuf2` |
| **Runtime** | `ffmpeg`, `ffprobe`, `ImageMagick` |

**Arch Linux**
```bash
sudo pacman -S libadwaita gtk4 glib2 pango graphene cairo gdk-pixbuf2 ffmpeg imagemagick
```

###### 
**Ubuntu / Debian**
```bash
sudo apt update
sudo apt install libadwaita-1-dev libgtk-4-dev libpango1.0-dev libgraphene-1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev ffmpeg imagemagick
```
###### 
**Fedora**
```bash
sudo dnf install libadwaita-devel gtk4-devel pango-devel graphene-devel cairo-devel gdk-pixbuf2-devel ffmpeg ImageMagick
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
