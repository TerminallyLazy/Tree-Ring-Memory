#!/bin/sh
set -eu

VERSION=${TREE_RING_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)}
[ "$VERSION" != "" ] || {
  printf '%s\n' "Could not determine Tree Ring Memory version" >&2
  exit 1
}

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m | tr '[:upper:]' '[:lower:]')
NAME="tree-ring-memory-$VERSION-$OS-$ARCH"
DIST_DIR=${TREE_RING_DIST_DIR:-dist}
WORK_DIR="$DIST_DIR/$NAME"
ARCHIVE="$NAME.tar.gz"

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

cargo build --release -p tree-ring-memory-cli --locked

cp target/release/tree-ring "$WORK_DIR/tree-ring"
cp README.md LICENSE install.sh "$WORK_DIR/"

tar -C "$DIST_DIR" -czf "$DIST_DIR/$ARCHIVE" "$NAME"

if command -v shasum >/dev/null 2>&1; then
  (
    cd "$DIST_DIR"
    shasum -a 256 "$ARCHIVE" > "$ARCHIVE.sha256"
    shasum -a 256 -c "$ARCHIVE.sha256"
  )
elif command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$DIST_DIR"
    sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"
    sha256sum -c "$ARCHIVE.sha256"
  )
else
  printf '%s\n' "warning: no SHA-256 tool found; checksum not written" >&2
fi

printf 'release_package=%s\n' "$DIST_DIR/$ARCHIVE"
