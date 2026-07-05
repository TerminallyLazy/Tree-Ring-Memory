#!/bin/sh
set -eu

REPO_URL=${TREE_RING_REPO:-"https://github.com/TerminallyLazy/Tree-Ring-Memory"}
GIT_REF=${TREE_RING_REF:-"main"}
INSTALL_SCOPE=${TREE_RING_INSTALL_SCOPE:-"global"}
INSTALL_DIR=${TREE_RING_INSTALL_DIR:-""}
SOURCE_DIR=${TREE_RING_SOURCE:-""}
ARCHIVE_URL=${TREE_RING_ARCHIVE_URL:-""}
ARCHIVE_SHA256=${TREE_RING_ARCHIVE_SHA256:-""}
MEMORY_ROOT=${TREE_RING_ROOT:-".tree-ring"}
RUN_INIT=${TREE_RING_INIT:-"0"}
RUN_ONBOARDING=${TREE_RING_ONBOARDING:-"1"}
ANIMATION=${TREE_RING_ANIMATION:-"1"}

if [ ! -t 1 ] || [ "${NO_COLOR:-}" != "" ]; then
  COLOR=0
else
  COLOR=1
fi

die() {
  printf '%s\n' "Tree Ring Memory install failed: $*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Tree Ring Memory installer

Usage:
  tmp=$(mktemp) && curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh -o "$tmp" && sh "$tmp"
  tmp=$(mktemp) && curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh -o "$tmp" && sh "$tmp" --project --init

Options:
  --global              Install to $HOME/.local/bin (default).
  --project             Install to .tree-ring/bin in the current project.
  --init                Run tree-ring welcome --init after install.
  --no-init             Do not initialize memory after install (default).
  --no-animation        Print stable output without animated rings.
  --no-onboarding       Skip tree-ring welcome after install.
  --install-dir DIR     Override install prefix. Binary goes in DIR/bin.
  --root DIR            Memory root used by onboarding/init (default .tree-ring).
  --repo URL            Git repository used by cargo install.
  --ref REF             Git branch used by cargo install (default main).
  --source DIR          Install from a local checkout instead of git.
  --archive-url URL     Install from a release tarball containing tree-ring.
  --archive-sha256 SUM  Required SHA-256 for --archive-url.
  -h, --help            Show this help.

Environment:
  TREE_RING_INSTALL_SCOPE=global|project
  TREE_RING_INSTALL_DIR=/path/to/prefix
  TREE_RING_ROOT=.tree-ring
  TREE_RING_REPO=https://github.com/TerminallyLazy/Tree-Ring-Memory
  TREE_RING_REF=main
  TREE_RING_SOURCE=/path/to/checkout
  TREE_RING_ARCHIVE_URL=https://example/tree-ring-memory.tar.gz
  TREE_RING_ARCHIVE_SHA256=...
  TREE_RING_INIT=1
  TREE_RING_ANIMATION=0
  TREE_RING_ONBOARDING=0
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --global)
      INSTALL_SCOPE=global
      ;;
    --project)
      INSTALL_SCOPE=project
      ;;
    --init)
      RUN_INIT=1
      ;;
    --no-init)
      RUN_INIT=0
      ;;
    --no-animation)
      ANIMATION=0
      ;;
    --no-onboarding)
      RUN_ONBOARDING=0
      ;;
    --install-dir)
      shift
      [ "$#" -gt 0 ] || die "--install-dir requires a value"
      INSTALL_DIR=$1
      ;;
    --root)
      shift
      [ "$#" -gt 0 ] || die "--root requires a value"
      MEMORY_ROOT=$1
      ;;
    --repo)
      shift
      [ "$#" -gt 0 ] || die "--repo requires a value"
      REPO_URL=$1
      ;;
    --ref)
      shift
      [ "$#" -gt 0 ] || die "--ref requires a value"
      GIT_REF=$1
      ;;
    --source)
      shift
      [ "$#" -gt 0 ] || die "--source requires a value"
      SOURCE_DIR=$1
      ;;
    --archive-url)
      shift
      [ "$#" -gt 0 ] || die "--archive-url requires a value"
      ARCHIVE_URL=$1
      ;;
    --archive-sha256)
      shift
      [ "$#" -gt 0 ] || die "--archive-sha256 requires a value"
      ARCHIVE_SHA256=$1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
  shift
done

if [ "$ARCHIVE_URL" != "" ] && [ "$ARCHIVE_SHA256" = "" ]; then
  die "--archive-sha256 is required with --archive-url"
fi

paint() {
  code=$1
  text=$2
  if [ "$COLOR" = "1" ]; then
    printf '\033[%sm%s\033[0m' "$code" "$text"
  else
    printf '%s' "$text"
  fi
}

line() {
  printf '%s\n' "$*"
}

ring_frame() {
  pulse=$1
  paint "38;5;37" "        .-=================-.        "
  line "$pulse"
  paint "38;5;204" "     .-'   cambium  fresh   '-.     "
  line ""
  paint "38;5;208" "   .'   .--- outer  detailed ---.   '."
  line ""
  paint "38;5;220" "  /   .'   .-- inner compressed --.  \\"
  line ""
  paint "38;5;33" " |   /   .' heartwood durable '.   | "
  line ""
  paint "38;5;204" "  \\   '.   scars visible seeds   .'  /"
  line ""
  paint "38;5;208" "   '.   '---.          .---'   .'   "
  line ""
  paint "38;5;37" "     '-.       '------'       .-'    "
  line ""
  paint "38;5;33" "        '==================='        "
  line ""
}

intro() {
  if [ "$ANIMATION" = "1" ] && [ "$COLOR" = "1" ]; then
    for pulse in "*" "+" "*" "+"; do
      printf '\033[2J\033[H'
      ring_frame "$pulse"
      sleep 0.08 2>/dev/null || :
    done
  else
    ring_frame "*"
  fi
  line ""
  paint "1" "Tree Ring Memory"
  line " installer"
  line "Framework-agnostic local memory for AI agents."
  line ""
}

require_cargo() {
  if [ "$ARCHIVE_URL" != "" ]; then
    return
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    die "cargo was not found. Install Rust from https://rustup.rs, then rerun this installer."
  fi
}

download() {
  url=$1
  output=$2
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$output"
  else
    die "curl or wget is required for --archive-url"
  fi
}

verify_sha256() {
  file=$1
  expected=$2
  [ "$expected" != "" ] || return
  if command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$file" | awk '{print $1}')
  elif command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$file" | awk '{print $1}')
  else
    die "SHA-256 verification requested, but shasum/sha256sum was not found"
  fi
  actual_lower=$(printf '%s' "$actual" | tr '[:upper:]' '[:lower:]')
  expected_lower=$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')
  [ "$actual_lower" = "$expected_lower" ] || die "archive checksum mismatch"
}

install_prefix() {
  if [ "$INSTALL_DIR" != "" ]; then
    printf '%s' "$INSTALL_DIR"
    return
  fi
  if [ "$INSTALL_SCOPE" = "project" ]; then
    printf '%s' ".tree-ring"
  else
    [ "${HOME:-}" != "" ] || die "HOME is not set. Pass --install-dir or use --project."
    printf '%s' "$HOME/.local"
  fi
}

install_binary() {
  prefix=$1
  mkdir -p "$prefix"
  line "Installing tree-ring into $prefix/bin"
  if [ "$ARCHIVE_URL" != "" ]; then
    tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/tree-ring-install.XXXXXX")
    archive="$tmp_dir/tree-ring-memory.tar.gz"
    download "$ARCHIVE_URL" "$archive"
    verify_sha256 "$archive" "$ARCHIVE_SHA256"
    tar -xzf "$archive" -C "$tmp_dir"
    binary=$(find "$tmp_dir" -type f -name tree-ring | head -n 1)
    [ "$binary" != "" ] || die "release archive did not contain tree-ring"
    mkdir -p "$prefix/bin"
    cp "$binary" "$prefix/bin/tree-ring"
    chmod +x "$prefix/bin/tree-ring"
    rm -rf "$tmp_dir"
  elif [ "$SOURCE_DIR" != "" ]; then
    line "Setting things up. This usually takes about 30 seconds after dependencies are cached."
    cargo install --path "$SOURCE_DIR/crates/tree-ring-memory-cli" --root "$prefix" --locked --force
  else
    line "Setting things up. This usually takes about 30 seconds after dependencies are cached."
    cargo install --git "$REPO_URL" --branch "$GIT_REF" --package tree-ring-memory-cli --root "$prefix" --locked --force
  fi
}

path_contains() {
  case ":$PATH:" in
    *":$1:"*) return 0 ;;
    *) return 1 ;;
  esac
}

intro
require_cargo

PREFIX=$(install_prefix)
BIN="$PREFIX/bin/tree-ring"

install_binary "$PREFIX"

[ -x "$BIN" ] || die "expected installed binary at $BIN"
"$BIN" --help >/dev/null || die "installed binary did not run"

line ""
paint "1" "Installed:"
line " $BIN"

if [ "$RUN_ONBOARDING" = "1" ]; then
  WELCOME_FLAGS=""
  if [ "$RUN_INIT" = "1" ]; then
    WELCOME_FLAGS="$WELCOME_FLAGS --init"
  fi
  if [ "$ANIMATION" != "1" ]; then
    WELCOME_FLAGS="$WELCOME_FLAGS --no-animation"
  fi
  # shellcheck disable=SC2086
  "$BIN" --root "$MEMORY_ROOT" welcome $WELCOME_FLAGS
fi

line ""
if [ "$INSTALL_SCOPE" = "global" ] && ! path_contains "$PREFIX/bin"; then
  line "Add this to your shell profile if tree-ring is not found:"
  line "  export PATH=\"$PREFIX/bin:\$PATH\""
fi
if [ "$INSTALL_SCOPE" = "project" ]; then
  line "For project-local use:"
  line "  export PATH=\"$PWD/$PREFIX/bin:\$PATH\""
fi
line "Open the TUI:"
line "  $BIN --root $MEMORY_ROOT tui"
