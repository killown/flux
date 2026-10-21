# Development

## Building from source

Flux links against GTK4, Libadwaita, and several C libraries. The build will fail on `pkg-config` errors if these are missing.

### Arch Linux

︙bash
sudo pacman -S base-devel git pkgconf rust \
 gtk4 libadwaita glib2 \
 poppler-glib fontconfig pango cairo gdk-pixbuf2 \
 vte4
︙

### Debian / Ubuntu

︙bash
sudo apt install build-essential pkg-config git curl \
 libgtk-4-dev libadwaita-1-dev libglib2.0-dev libglib2.0-dev-bin \
 libpoppler-glib-dev libfontconfig1-dev \
 libpango1.0-dev libcairo2-dev \
 libgdk-pixbuf-2.0-dev \
 libvte-2.91-gtk4-dev
︙

### Fedora

︙bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config git \
 gtk4-devel libadwaita-devel glib2-devel \
 poppler-glib-devel fontconfig-devel pango-devel cairo-devel \
 gdk-pixbuf2-devel \
 vte291-gtk4-devel
︙

### Build

︙bash
git clone <https://github.com/killown/flux>
cd flux
cargo build --release
︙

The binary is at `target/release/flux-fm`.

## Regenerating documentation

The files under `docs/` (except `index.md`, `SUMMARY.md`, and `development.md`) are generated from source. If you change CLI options, config fields, built-in actions, search syntax, or CSS variables, regenerate them:

︙bash
cargo run --bin gen-docs
︙

CI runs `cargo run --bin gen-docs` and fails if `git diff --exit-code docs/` shows drift. This keeps the docs, the `--help` output, and the code in sync.

## Optional dependencies

These are not needed to build, but extend what Flux can preview or open. Missing tools are logged to stderr at startup.

| Tool                            | Enables                             |
| ------------------------------- | ----------------------------------- |
| `ffmpeg`, `ffprobe`             | Video thumbnails, media duration    |
| `7z` (`p7zip-full` / `p7zip`)   | AppImage icons, UDF ISO support     |
| `unar` or `unrar`               | RAR archive browsing                |
| `exiftool`                      | RAW camera thumbnails (primary)     |
| `raw-thumbnailer` / `miniaturo` | RAW camera thumbnails (fallback)    |
| `wrestool`, `icotool`           | `.exe` icon extraction              |
| `mpv`                           | Audio playback in the preview panel |
| `soffice` / `libreoffice`       | Office document previews            |
| `fc-cache`                      | Font install refresh                |

## Development profile

The `local` profile (`Cargo.toml`) inherits from `dev` but builds at `opt-level = 2`:

︙bash
cargo build --profile local
︙

A helper script at `tools/local-build.sh` does this, then regenerates docs and installs:

︙bash
./tools/local-build.sh
︙
