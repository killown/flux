#!/bin/bash
set -e

cd "$(dirname "$0")/../.."

cargo build --release

APPIMG_DIR="packaging/appimg"
APPDIR="$APPIMG_DIR/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/512x512/apps"

cp target/release/flux-fm "$APPDIR/usr/bin/flux-fm"
cp "$APPIMG_DIR/io.github.killown.flux.desktop" "$APPDIR/usr/share/applications/"
cp "$APPIMG_DIR/AppRun" "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"

rsvg-convert -h 512 flux.svg -o "$APPDIR/io.github.killown.flux.png"
magick "$APPDIR/io.github.killown.flux.png" -background none -gravity center -extent 512x512 "$APPDIR/io.github.killown.flux.png"
cp "$APPDIR/io.github.killown.flux.png" "$APPDIR/usr/share/icons/hicolor/512x512/apps/"

cd "$APPIMG_DIR"

if [ ! -f linuxdeploy-x86_64.AppImage ]; then
  wget -q https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
  chmod +x linuxdeploy-x86_64.AppImage
fi
if [ ! -f appimagetool-x86_64.AppImage ]; then
  wget -q https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x appimagetool-x86_64.AppImage
fi

./linuxdeploy-x86_64.AppImage \
  --appdir AppDir \
  --executable AppDir/usr/bin/flux-fm \
  --desktop-file AppDir/usr/share/applications/io.github.killown.flux.desktop \
  --icon-file AppDir/io.github.killown.flux.png

rm -f AppDir/AppRun.wrapped
rm -rf AppDir/apprun-hooks

ARCH=x86_64 ./appimagetool-x86_64.AppImage --comp zstd AppDir Flux-x86_64.AppImage
