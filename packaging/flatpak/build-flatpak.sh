#!/bin/sh
set -eu

APP_ID=io.github.dral.oxidisc
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
MANIFEST="$ROOT/packaging/flatpak/${APP_ID}.yml"
BUILD_DIR="$ROOT/target/flatpak/build"
REPO_DIR="$ROOT/target/flatpak/repo"
BUNDLE="$ROOT/target/flatpak/${APP_ID}.flatpak"

cd "$ROOT"

if ! command -v flatpak-builder >/dev/null 2>&1; then
  echo "flatpak-builder is required to build the Flatpak." >&2
  exit 1
fi

mkdir -p "$ROOT/target/flatpak"

flatpak-builder \
  --force-clean \
  --user \
  --install-deps-from=flathub \
  --repo="$REPO_DIR" \
  "$BUILD_DIR" \
  "$MANIFEST"

if command -v flatpak >/dev/null 2>&1; then
  flatpak build-bundle "$REPO_DIR" "$BUNDLE" "$APP_ID"
  echo "Flatpak bundle written to $BUNDLE"
else
  echo "Flatpak repository written to $REPO_DIR"
  echo "Install flatpak to produce $BUNDLE"
fi
