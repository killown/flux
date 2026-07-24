# 🌊 Flux

![Rust](https://img.shields.io/badge/Rust-1.75%2B-black?logo=rust)
![GTK4](https://img.shields.io/badge/GTK-4-green?logo=gnome)
![License](https://img.shields.io/badge/License-GPLv3-blue.svg)

**Flux** is a minimalist, high-performance file manager for Linux. It’s built for those who love the clean look of GNOME but want a tool that stays out of their way.

<img width="1920" height="1080" alt="screenshot" src="https://github.com/user-attachments/assets/019f362a-2de6-4f6c-94f2-891564fee863" />

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

## ⌨️ Keyboard Shortcut & Input Hub Reference

### 6.1 Unified Global Bindings
The following table details the core hotkey mappings defined across `src/ui/inputs.rs`, `src/utils/helpers.rs`, and the shortcut controller stack:

| Key Combination | Message Reference / Operation Target | Functional System Description |
| :--- | :--- | :--- |
| `F1` | `AppMsg::ShowHelp` | Opens the graphical keyboard shortcut overlay window. |
| `F2` | `AppMsg::TriggerRenameSelection` | Puts the selected item into inline name editing mode. |
| `F3` | `AppMsg::ShowIconPicker` | Opens a custom icon picker dialog for the current directory. |
| `F4` | `AppMsg::ToggleTerminal` | Toggles the visibility of the embedded virtual terminal panel. |
| `F5` | `AppMsg::Refresh` | Forces a full reload of the current directory from disk. |
| `F8` | `AppMsg::ToggleSidebar` | Toggles the left navigation sidebar tray. |
| `F9` | `app.open-menu-editor` | Spawns the runtime context menu configuration editor window. |
| `F10` | `app.open-settings` | Launches the central preference settings interface panel. |
| `Return` / `KP_Enter` | `AppMsg::Activate` | Opens selected files or navigates into selected folders. |
| `Backspace` | `AppMsg::GoBack` | Navigates back one step in the directory history log. |
| `Alt + Right` | `AppMsg::GoForward` | Moves forward one step in the directory history stack. |
| `Ctrl + f` | `AppMsg::SwitchHeader("search")` | Focuses the entry line and switches the interface to filtering mode. |
| `Ctrl + h` | `AppMsg::ToggleHidden` | Toggles the visibility of hidden files and folders. |
| `Ctrl + s` | `AppMsg::CycleSort` | Rotates through the available sorting criteria. |
| `Ctrl + Shift + s` | `AppMsg::ToggleSortOrder` | Toggles between ascending and descending sort order. |
| `Shift + s` | `AppMsg::CycleFolderPriority` | Toggles whether folders are grouped above files when sorting. |
| `Insert` | `AppMsg::AddExclusive(None)` | Adds the selected path to the temporary quick list cache. |
| `Tab` | `AppMsg::NextExclusive` | Cycles forward to the next folder in the temporary quick list. |
| `Ctrl + Tab` | `AppMsg::PrevExclusive` | Cycles backward to the previous folder in the temporary quick list. |
| `Ctrl + Insert` | `AppMsg::AddToSidebarPermanent` | Pins the current directory to the configuration bookmarks permanently. |
| `Ctrl + Delete` | `AppMsg::ClearExclusive` | Clears all entries from the temporary quick list cache. |

## Configuration & Customization

Everything is managed via `~/.config/flux/config.toml`. Want to add a custom folder to your sidebar or change the default sorting? Just edit the text file. It's that simple.

> **Custom Actions:** You can define your own right-click commands. Add "Open in VS Code" or "Optimize Image" by linking a simple shell command in your config.

## Extra Themes
<img width="1080" height="608" alt="demo" src="https://github.com/user-attachments/assets/b4ca8e9c-ec4b-47bf-a560-be88086df10e" />
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
