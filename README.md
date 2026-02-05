# 🌊 Flux

**Flux** is a minimalist, high-performance file manager for Linux. It’s built for those who love the clean look of GNOME but want a tool that stays out of their way. No cluttered toolbars, no hidden mazes of settings—just you and your files.

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

**Navigate Back/Forward**

`Alt + Left/Right`

## Configuration & Customization

Everything is managed via `~/.config/flux/config.toml`. Want to add a custom folder to your sidebar or change the default sorting? Just edit the text file. It's that simple.

> **Custom Actions:** You can define your own right-click commands. Add "Open in VS Code" or "Optimize Image" by linking a simple shell command in your config.

## Tech Stack

- **Language:** Rust 1.75+ (Memory safe and blazing fast)
- **UI Framework:** Relm4 & GTK4 (Native GNOME experience)
- **Runtime:** Tokio & Futures (Non-blocking I/O)

## Getting Started

git clone https://github.com/yourusername/flux.git
cd flux
cargo run --release

## License

Flux is free and open-source software licensed under the **GPLv3**.
