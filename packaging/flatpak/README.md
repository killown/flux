# Flux Flatpak

## Prerequisites

Install the GNOME 49 platform and SDK along with the corresponding Rust SDK extension:

```bash
flatpak install --user flathub org.gnome.Platform//49 org.gnome.Sdk//49
flatpak install --user flathub org.freedesktop.Sdk.Extension.rust-stable//25.08
```

## Generate cargo-sources.json

Flatpak builds offline, so all Cargo dependencies must be pre-fetched into a sources manifest. Use [flatpak-cargo-generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo):

```bash
pip install aiohttp toml tomlkit
python3 flatpak-cargo-generator.py ../../Cargo.lock -o cargo-sources.json
```

If `Cargo.lock` is not yet present or updated, generate it from the repository root first:

```bash
cargo fetch
```

## Build and install locally

Build and register the application in the user installation:

```bash
flatpak-builder --user --install --force-clean build-dir io.github.killown.flux.yml
```

## Run

Launch the application using its reverse-DNS application ID:

```bash
flatpak run io.github.killown.flux
```

## Notes

- `--filesystem=host` is required: sandboxing a full file manager without host access breaks local filesystem navigation, trash handling, and xattr tagging.
- `--talk-name=org.gtk.vfs.*` and `--filesystem=xdg-run/gvfs` provide access to remote network shares, MTP devices, and the system trash via GVfs.
- Menus, custom themes, utility scripts, and compiled gettext translations (`po/pt_BR.po`) are staged into `${FLATPAK_DEST}/share/flux/` and `${FLATPAK_DEST}/share/locale/` during the build phase.
- For upstream Flathub submission, valid `<screenshots>` tags hosting images at stable public URLs are required in `io.github.killown.flux.metainfo.xml`.
