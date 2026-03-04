# 🌊 Flux

**Flux** is a minimalist, high-performance file manager for Linux. It’s built for those who love the clean look of GNOME but want a tool that stays out of their way.

<img width="1262" height="749" alt="focused_view" src="https://github.com/user-attachments/assets/17f4e6c0-2ca8-4a81-bc16-cde9d2dfd05d" />

<img width="1244" height="879" alt="focused_view" src="https://github.com/user-attachments/assets/479ed40e-0d26-48a3-b087-7e5f0dca8b6c" />

## Why Flux?

Most file managers try to do everything. Flux tries to do _one thing_ perfectly: letting you browse your data at light speed without the visual noise.

- **Zero Clutter:** We replaced bulky buttons with a smart, dynamic header that shows you exactly what you need to know.
- **Asynchronous Heart:** Got a folder with 5,000 high-res wallpapers? Flux won't sweat. It uses a throttled async pipeline to load thumbnails without ever freezing the window.
- **Shortcut First:** Flux is designed for power users who prefer the keyboard over hunting for tiny icons.

## Power User Shortcuts

Flux is built around the "Config-Driven" philosophy. You control the logic, we provide the speed.

Action

Shortcut

**Cycle Sort Mode** (Name → Date → Size)

`Ctrl + S`

**Toggle Hidden Files**

`Ctrl + H`

**Zoom Icons** (Smooth Scaling)

`Ctrl + Mouse Wheel`

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

## Getting Started

    # required:
    -ladwaita-1 → libadwaita
    -lgtk-4 → GTK 4
    -lgdk_pixbuf-2.0 → GdkPixbuf
    -lcairo-gobject → Cairo
    -lgraphene-1.0 → Graphene

    git clone https://github.com/killown/flux.git
    cd flux
    cargo run --release

## License

Flux is free and open-source software licensed under the **GPLv3**.
