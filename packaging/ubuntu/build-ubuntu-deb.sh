#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
BUILD_ROOT="$ROOT/target/ubuntu/source" \
PACKAGE_CONTEXT="Ubuntu" \
exec "$ROOT/packaging/debian/build-debian-deb.sh" "$@"
