#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
BUILD_ROOT="${BUILD_ROOT:-$ROOT/target/debian/source}"
ARTIFACT_ROOT="$(dirname -- "$BUILD_ROOT")"
PACKAGE_CONTEXT="${PACKAGE_CONTEXT:-Debian}"
DEBIAN_DIR="$ROOT/packaging/debian"

cd "$ROOT"

if ! command -v dpkg-buildpackage >/dev/null 2>&1; then
  echo "dpkg-buildpackage is required to build the Debian/Ubuntu .deb package." >&2
  exit 1
fi

for file in changelog control copyright rules source/format watch; do
  if [ ! -e "$DEBIAN_DIR/$file" ]; then
    echo "Missing Debian package metadata: $DEBIAN_DIR/$file" >&2
    exit 1
  fi
done

rm -rf "$BUILD_ROOT"
mkdir -p "$BUILD_ROOT/debian"

tar \
  --exclude='.git' \
  --exclude='target' \
  -cf - \
  . | tar -xf - -C "$BUILD_ROOT"

cp "$DEBIAN_DIR/changelog" "$BUILD_ROOT/debian/changelog"
cp "$DEBIAN_DIR/control" "$BUILD_ROOT/debian/control"
cp "$DEBIAN_DIR/copyright" "$BUILD_ROOT/debian/copyright"
cp "$DEBIAN_DIR/rules" "$BUILD_ROOT/debian/rules"
cp "$DEBIAN_DIR/watch" "$BUILD_ROOT/debian/watch"
mkdir -p "$BUILD_ROOT/debian/source"
cp "$DEBIAN_DIR/source/format" "$BUILD_ROOT/debian/source/format"
chmod 755 "$BUILD_ROOT/debian/rules"

cd "$BUILD_ROOT"
dpkg-buildpackage -us -uc -b

echo "$PACKAGE_CONTEXT package artifacts written to $ARTIFACT_ROOT"
echo ""
echo "Upload/contrib:"
echo "  Test the .deb on a clean matching Debian or Ubuntu GNOME desktop before publishing."
echo "  Do not upload local .deb artifacts to a PPA; PPAs build from signed source uploads."
echo "  See Distribution Instructions.md for GitHub Release and Launchpad/PPA steps."
