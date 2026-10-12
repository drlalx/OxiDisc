#!/bin/sh
set -eu

APP=oxidisc
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
VERSION="$(awk -F'"' '/^version = / { print $2; exit }' "$ROOT/Cargo.toml")"
RPM_ROOT="$ROOT/target/fedora/rpmbuild"
SPEC="$ROOT/packaging/fedora/${APP}.spec"
SOURCE="$RPM_ROOT/SOURCES/${APP}-${VERSION}.tar.gz"

cd "$ROOT"

if ! command -v rpmbuild >/dev/null 2>&1; then
  echo "rpmbuild is required to build the Fedora RPM." >&2
  exit 1
fi

if [ -d "$RPM_ROOT" ] && find "$RPM_ROOT" -mindepth 1 -print -quit | grep -q .; then
  if [ -t 0 ]; then
    printf 'Existing Fedora build artifacts found in %s\nKeep them? [Y/n] ' "$RPM_ROOT" >&2
    read -r KEEP_EXISTING
    case "$KEEP_EXISTING" in
      ''|[Yy]|[Yy][Ee][Ss])
        ;;
      [Nn]|[Nn][Oo])
        rm -rf "$RPM_ROOT"
        ;;
      *)
        echo "Please answer y or n." >&2
        exit 1
        ;;
    esac
  else
    echo "Existing Fedora build artifacts found in $RPM_ROOT; keeping them." >&2
  fi
fi

mkdir -p \
  "$RPM_ROOT/BUILD" \
  "$RPM_ROOT/BUILDROOT" \
  "$RPM_ROOT/RPMS" \
  "$RPM_ROOT/SOURCES" \
  "$RPM_ROOT/SPECS" \
  "$RPM_ROOT/SRPMS" \
  "$RPM_ROOT/TMP"

tar \
  --exclude='.git' \
  --exclude='target' \
  --transform="s|^|oxidisc-${VERSION}/|" \
  -czf "$SOURCE" \
  .

rpmbuild \
  --define "_topdir $RPM_ROOT" \
  --define "_tmppath $RPM_ROOT/TMP" \
  -ba "$SPEC"

echo "RPM artifacts written under $RPM_ROOT/RPMS and $RPM_ROOT/SRPMS"
