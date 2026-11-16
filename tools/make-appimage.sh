#!/bin/sh
# Packs the Linux release programs into one OpenMCM2 AppImage, with its
# icon and menu entry: a single file to download, make executable and
# start (or add to Steam on a Steam Deck).
# Usage: sh tools/make-appimage.sh <folder with the programs> <out.AppImage>
set -e
bin=$1
out=$2
[ -n "$bin" ] && [ -n "$out" ] || { echo "usage: $0 <bin folder> <out.AppImage>" >&2; exit 2; }
root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app="$work/OpenMCM2.AppDir"
mkdir -p "$app/usr/bin" "$app/usr/share/applications" "$app/usr/share/icons/hicolor/256x256/apps"
cp "$bin/mm2-launcher" "$bin/mm2-viewer" "$bin/mm2-tools" "$app/usr/bin/"
cp "$root/assets/linux/AppRun" "$app/AppRun"
chmod +x "$app/AppRun" "$app/usr/bin/"*
cp "$root/assets/linux/openmcm2.desktop" "$app/openmcm2.desktop"
cp "$root/assets/linux/openmcm2.desktop" "$app/usr/share/applications/"
cp "$root/assets/icon/icon-256.png" "$app/openmcm2.png"
cp "$root/assets/icon/icon-256.png" "$app/usr/share/icons/hicolor/256x256/apps/openmcm2.png"
ln -s openmcm2.png "$app/.DirIcon"
mkdir -p "$app/usr/share/doc/openmcm2"
cp "$root/README.md" "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$root/crates/mm2-indeo/COPYING.LGPLv2.1" "$app/usr/share/doc/openmcm2/"

# appimagetool 1.9.0, checked against its known SHA-256. It runs
# unpacked, as build machines and containers often have no FUSE.
tool="$work/appimagetool"
curl -sSfL -o "$tool" https://github.com/AppImage/appimagetool/releases/download/1.9.0/appimagetool-x86_64.AppImage
echo "46fdd785094c7f6e545b61afcfb0f3d98d8eab243f644b4b17698c01d06083d1  $tool" | sha256sum -c -
chmod +x "$tool"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$app" "$out"
ls -l "$out"
