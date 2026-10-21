# Development

## Building from Source

Flux links against GTK4, Libadwaita, and several C libraries. The build will fail with `pkg-config` errors if these dependencies are missing.

### Arch Linux

```bash
sudo pacman -S base-devel git pkgconf rust \
  gtk4 libadwaita glib2 \
  poppler-glib fontconfig pango cairo gdk-pixbuf2 \
  vte4
```

### Debian / Ubuntu

```bash
sudo apt install build-essential pkg-config git curl \
  libgtk-4-dev libadwaita-1-dev libglib2.0-dev libglib2.0-dev-bin \
  libpoppler-glib-dev libfontconfig1-dev \
  libpango1.0-dev libcairo2-dev \
  libgdk-pixbuf-2.0-dev \
  libvte-2.91-gtk4-dev
```

### Fedora

```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config git \
  gtk4-devel libadwaita-devel glib2-devel \
  poppler-glib-devel fontconfig-devel pango-devel cairo-devel \
  gdk-pixbuf2-devel \
  vte291-gtk4-devel
```

### Build

```bash
git clone https://github.com/killown/flux.git
cd flux
cargo build --release
```

The compiled binary is located at `target/release/flux-fm`.

## Regenerating Documentation

The files under `docs/` are generated from source, except for `index.md`, `SUMMARY.md`, and `development.md`.

If you change CLI options, configuration fields, built-in actions, search syntax, or CSS variables, regenerate the documentation:

```bash
cargo run --bin gen-docs
```

CI runs `cargo run --bin gen-docs` and fails if `git diff --exit-code docs/` detects changes. This keeps the documentation, `--help` output, and source code in sync.

## Optional Dependencies

These tools are not required to build Flux, but they extend its preview and file-opening capabilities. Missing tools are logged to stderr at startup.

| Tool                            | Enables                                           |
| ------------------------------- | ------------------------------------------------- |
| `ffmpeg`, `ffprobe`             | Video thumbnails and media duration               |
| `7z` (`p7zip-full` / `p7zip`)   | AppImage icons and UDF ISO support                |
| `unar` or `unrar`               | RAR archive browsing                              |
| `exiftool`                      | Primary RAW camera thumbnail support              |
| `raw-thumbnailer` / `miniaturo` | Fallback RAW camera thumbnail support             |
| `wrestool`, `icotool`           | `.exe` icon extraction                            |
| `mpv`                           | Audio playback in the preview panel               |
| `soffice` / `libreoffice`       | Office document previews                          |
| `fc-cache`                      | Refreshing the font cache after font installation |

## Development Profile

The `local` profile in `Cargo.toml` inherits from `dev` but uses `opt-level = 2` for optimized builds.

Build using this profile:

```bash
cargo build --profile local
```

The helper script `tools/local-build.sh` builds with this profile, regenerates the documentation, and installs Flux:

```bash
./tools/local-build.sh
```
