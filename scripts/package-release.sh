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

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

cargo build --release -p tree-ring-memory-cli --locked

cp target/release/tree-ring "$WORK_DIR/tree-ring"
cp README.md LICENSE install.sh "$WORK_DIR/"

tar -C "$DIST_DIR" -czf "$DIST_DIR/$NAME.tar.gz" "$NAME"

if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$DIST_DIR/$NAME.tar.gz" > "$DIST_DIR/$NAME.tar.gz.sha256"
elif command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$DIST_DIR/$NAME.tar.gz" > "$DIST_DIR/$NAME.tar.gz.sha256"
else
  printf '%s\n' "warning: no SHA-256 tool found; checksum not written" >&2
fi

printf 'release_package=%s\n' "$DIST_DIR/$NAME.tar.gz"
