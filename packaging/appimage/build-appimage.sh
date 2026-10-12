#!/bin/sh
set -eu

APP=oxidisc
VERSION=2.0.0
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
APPDIR="$ROOT/target/appimage/${APP}.AppDir"

cd "$ROOT"
cargo build --release --locked --features "egui-ui tui"

rm -rf "$APPDIR"
install -Dm755 target/release/oxidisc "$APPDIR/usr/bin/oxidisc"
install -Dm755 packaging/appimage/AppRun "$APPDIR/AppRun"
install -Dm644 resources/oxidisc.desktop "$APPDIR/${APP}.desktop"
install -Dm644 resources/oxidisc.desktop "$APPDIR/usr/share/applications/${APP}.desktop"
install -Dm644 resources/images/oxidisc.png "$APPDIR/io.github.dral.oxidisc.png"
install -Dm644 resources/images/oxidisc.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/io.github.dral.oxidisc.png"
install -Dm644 resources/metainfo/io.github.dral.oxidisc.metainfo.xml "$APPDIR/usr/share/metainfo/io.github.dral.oxidisc.metainfo.xml"
install -Dm644 LICENSE "$APPDIR/usr/share/licenses/${APP}/LICENSE"

if command -v appimagetool >/dev/null 2>&1; then
  ARCH=x86_64 VERSION="$VERSION" appimagetool "$APPDIR" "$ROOT/target/appimage/${APP}-${VERSION}-x86_64.AppImage"
else
  echo "AppDir prepared at $APPDIR"
  echo "Install appimagetool to produce target/appimage/${APP}-${VERSION}-x86_64.AppImage"
fi

echo ""
echo "Upload/contrib: AppImage"
echo "  Test the AppImage on a clean Linux desktop before publishing."
echo "  Upload the final AppImage to the GitHub release only after release approval."
echo "  See Distribution Instructions.md for release and checksum steps."
