# #Flux

Flux is a minimalist, high-performance file manager for Linux, built with Rust, GTK4, and Libadwaita. It is designed for users who want a clean, GNOME-integrated experience without the clutter of traditional toolbars and complex preference menus.

## The Philosophy

The core goal of Flux is to maintain visual simplicity while providing infinite flexibility. Instead of building heavy widgets for sorting, filtering, and view toggles, Flux relies entirely on its configuration file. If you want to change how files are displayed, sorted, or handled, you do it in the config—not through a maze of buttons.

- Logic over Clutter: No bulky toolbars or complex sorting menus.
- Asynchronous Heart: Optimized to handle massive directories (like 3GB wallpaper folders) using a throttled async thumbnail pipeline that never freezes the UI.
- Native Feel: Built specifically to follow the GNOME Human Interface Guidelines (HIG).

## Planned Features

### Config-Driven Experience

Everything from the default sort order to the thumbnail concurrency limits will be managed via a central configuration file. This keeps the UI focused purely on your files.

### Custom Context Menus

A powerful right-click menu system is planned that will allow you to define Custom Actions.

- Want a "Set as Wallpaper" button? Define it in the config.
- Need to "Open in VS Code" or "Optimize Image"? Just add the shell command to your custom actions list.

### Smart Previews

Fast, non-blocking thumbnail generation for images and high-fidelity file type icons.

## Architecture

Flux is built on a modern stack for maximum reliability:

- Language: Rust
- Framework: Relm4 (Idiomatic GTK4)
- Concurrency: Tokio & Futures for non-blocking I/O.
- Styling: Native CSS support for deep customization.

## Getting Started

### Prerequisites

- Rust (Latest Stable)
- GTK4 & Libadwaita development headers

### Installation

    git clone https://github.com/yourusername/flux.git
    cd flux
    cargo run

## License

This project is licensed under the GPLv3 License.
