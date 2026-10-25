# Flux

A keyboard-first Linux file manager built with GTK4, Libadwaita, and Relm4.

Flux is designed as a highly responsive, native file manager. It avoids blocking the main GTK thread by offloading all I/O, directory reads, and background processing to isolated thread pools.

## Architecture at a glance

- **UI:** GTK4 + Libadwaita via Relm4 (MVU pattern).
- **Parallelism:** Capped Rayon pool for directory scans, sorting, and metadata extraction.
- **Async I/O:** Tokio for subprocesses, network operations, and file transfers.
- **State:** SQLite (`rusqlite`) for view settings, tags, folder icons, and location history.
- **Search:** Live Nucleo fuzzy filter over the current directory; optional SQLite FTS5 trigram index for recursive filename searches; Aho-Corasick content scanner.
- **Archives:** Native Rust for zip/tar/7z, with external tools for rar/deb/udf-iso.

## Documentation

- **[CLI Reference](cli.md)** – How to launch Flux and manage custom icons.
- **[Configuration](config.md)** – Tweak `~/.config/flux/config.toml` to your liking.
- **[Custom Actions](actions.md)** – Add your own right-click menu entries.
- **[Search Syntax](search.md)** – Master the live filter, recursive search, and content search.
- **[Features](features.md)** - What Flux does, grouped by area, with links to the reference pages.
- **[Shell Integration](shell-integration.md)** – Use the selection and current folder from your shell.
- **[CSS Variables](css-variables.md)** – Theme Flux to match your desktop.

## Source Code

Flux is open source. You can find the repository and report issues on [GitHub](https://github.com/killown/flux).
