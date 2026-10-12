#!/bin/sh
set -eu
export cd_cmd=cd

APP=oxidisc
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
VERSION="$(awk -F'"' '/^version = / { print $2; exit }' "$ROOT/Cargo.toml")"
OUTPUT="${APP}_${VERSION}_amd64.snap"
OUTPUT_DIR="$ROOT/target/snap"

cd "$ROOT"

if ! command -v snapcraft >/dev/null 2>&1; then
  echo "snapcraft is required to build the Snap package." >&2
  exit 1
fi

if [ ! -d "$OUTPUT_DIR" ]; then

  mkdir -p "$OUTPUT_DIR"

  else

    echo "Output directory $OUTPUT_DIR already exists." >&2

    echo "Removing existing output directory $OUTPUT_DIR...WAIT ONE..." >&2 

    sleep 2

    echo "Continuing with build..." >&2

    sleep 1

    rm -rf "$OUTPUT_DIR" >&2

    mkdir -p "$OUTPUT_DIR" >&2

fi


snapcraft clean
snapcraft pack --output "$OUTPUT_DIR/$OUTPUT"

if  [ -f "$OUTPUT_DIR/$OUTPUT" ]; then

  echo "Snap package written to" "$OUTPUT_DIR/$OUTPUT"

else

  echo "snapcraft did not write the expected Snap package to" "$OUTPUT_DIR/" >&2
  exit 1

fi
