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
ANIMATION=${TREE_RING_ANIMATION:-"auto"}
UPDATE_PATH=${TREE_RING_UPDATE_PATH:-"1"}
PATH_PROFILE_UPDATED=""

if [ ! -t 1 ] || [ "${NO_COLOR:-}" != "" ]; then
  COLOR=0
else
  COLOR=1
fi

case "$ANIMATION" in
  auto|0|1) ;;
  *) ANIMATION=auto ;;
esac

if [ "$ANIMATION" = "auto" ]; then
  if [ -t 1 ] && [ "$COLOR" = "1" ] && [ "${TERM:-}" != "dumb" ]; then
    ANIMATION=1
  else
    ANIMATION=0
  fi
fi

die() {
  printf '%s\n' "Tree Ring Memory install failed: $*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Tree Ring Memory installer

Usage:
  installer=$(mktemp) && curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh -o "$installer" && sh "$installer"
  installer=$(mktemp) && curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh -o "$installer" && sh "$installer" --project --init

Options:
  --global              Install to $HOME/.local/bin (default).
  --project             Install to .tree-ring/bin in the current project.
  --init                Run tree-ring welcome --init after install.
  --no-init             Do not initialize memory after install (default).
  --animation           Play the installer ring animation when stdout is a TTY.
  --no-animation        Print stable output without animated rings.
  --no-onboarding       Skip tree-ring welcome after install.
  --no-path-update      Do not append the global install bin dir to a shell profile.
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
  TREE_RING_ANIMATION=auto|1|0
  TREE_RING_ONBOARDING=0
  TREE_RING_UPDATE_PATH=0
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
    --animation)
      ANIMATION=1
      ;;
    --no-animation)
      ANIMATION=0
      ;;
    --no-onboarding)
      RUN_ONBOARDING=0
      ;;
    --no-path-update)
      UPDATE_PATH=0
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

paint_line() {
  paint "$1" "$2"
  line ""
}

term_cols() {
  if command -v tput >/dev/null 2>&1; then
    cols=$(tput cols 2>/dev/null || printf '80')
  else
    cols=80
  fi
  case "$cols" in
    ''|*[!0-9]*) printf '80' ;;
    *) printf '%s' "$cols" ;;
  esac
}

can_animate() {
  [ "$ANIMATION" = "1" ] || return 1
  [ -t 1 ] || return 1
  [ "${TERM:-}" != "dumb" ] || return 1
  cols=$(term_cols)
  [ "$cols" -ge 72 ] || return 1
  sleep 0.001 2>/dev/null || return 1
  return 0
}

sleep_tick() {
  sleep 0.08 2>/dev/null || true
}

clear_ring_frame() {
  printf '\033[%sA\033[J' "$1"
}

ring_frame() {
  phase=${1:-0}
  case "$phase" in
    0) core="oo"; glow="." ;;
    1) core="OO"; glow="*" ;;
    2) core="@@"; glow="o" ;;
    *) core="OO"; glow="*" ;;
  esac

  paint_line "1;38;5;24"  "                    .----------------------.                    "
  paint_line "1;38;5;37"  "                .-'  .----------------.  /'-.                  "
  paint_line "1;38;5;204" "             .-'  .-'  .------------.  /  . '-.                "
  paint_line "1;38;5;208" "           .'  .-'  .-'   .------.   /  .' '.  '.              "
  paint_line "1;38;5;220" "          /  .'   .'    .' .----. '. / .'    '.  \\             "
  paint_line "1;38;5;208" "         |  /    /     /  / ${core} \\  V /       |  |            "
  paint_line "1;38;5;94"  "         | |    |     |  |  ${glow}${glow}  |  |        |  |            "
  paint_line "1;38;5;208" "         |  \\    \\     \\  \\____/  /\\        |  |            "
  paint_line "1;38;5;220" "          \\  '.   '.    '.______.'  '.      /  /             "
  paint_line "1;38;5;208" "           '.  '-.  '-.              .'-.__.' .'              "
  paint_line "1;38;5;204" "             '-.  '-.  '------------'  .-'.-'                "
  paint_line "1;38;5;37"  "                '-.  '----------------' .-'                  "
  paint_line "1;38;5;24"  "                   '--------------------'                    "
  paint_line "1;38;5;37"  "        /|                                            |\\       "
  paint_line "1;38;5;204" "       / |              TREE RING MEMORY              | \\      "
  paint_line "1;38;5;208" "      /__|____________________________________________|__\\     "
  paint_line "38;5;244"   "             fresh rings -> scars -> heartwood                 "
}

tree_ring_animation() {
  frame_lines=17
  printed=0

  # Hide the cursor during the short installer splash. The trap restores it if
  # the install is interrupted before the final frame is printed.
  printf '\033[?25l'
  trap 'printf "\033[?25h"; exit 1' INT TERM
  trap 'printf "\033[?25h"' EXIT

  for frame in 0 1 2 3 2 1 0; do
    if [ "$printed" = "1" ]; then
      clear_ring_frame "$frame_lines"
    fi
    ring_frame "$frame"
    printed=1
    sleep_tick
  done

  printf '\033[?25h'
  trap - INT TERM EXIT
  line ""
}

intro() {
  if [ "$RUN_ONBOARDING" != "1" ]; then
    if can_animate; then
      tree_ring_animation
    else
      ring_frame 3
    fi
    line ""
  fi

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
    cargo install --git "$REPO_URL" --branch "$GIT_REF" tree-ring-memory-cli --root "$prefix" --locked --force
  fi
}

path_contains() {
  case ":$PATH:" in
    *":$1:"*) return 0 ;;
    *) return 1 ;;
  esac
}

shell_quote() {
  printf "'"
  printf '%s' "$1" | sed "s/'/'\\\\''/g"
  printf "'"
}

shell_profile_path() {
  [ "${HOME:-}" != "" ] || return 1
  shell_name=$(basename "${SHELL:-}")
  case "$shell_name" in
    zsh)
      [ -f "$HOME/.zshrc" ] && { printf '%s' "$HOME/.zshrc"; return 0; }
      [ -f "$HOME/.zprofile" ] && { printf '%s' "$HOME/.zprofile"; return 0; }
      printf '%s' "$HOME/.zshrc"
      ;;
    bash)
      [ -f "$HOME/.bashrc" ] && { printf '%s' "$HOME/.bashrc"; return 0; }
      [ -f "$HOME/.bash_profile" ] && { printf '%s' "$HOME/.bash_profile"; return 0; }
      printf '%s' "$HOME/.bashrc"
      ;;
    *)
      [ -f "$HOME/.profile" ] && { printf '%s' "$HOME/.profile"; return 0; }
      [ -f "$HOME/.zshrc" ] && { printf '%s' "$HOME/.zshrc"; return 0; }
      [ -f "$HOME/.bashrc" ] && { printf '%s' "$HOME/.bashrc"; return 0; }
      printf '%s' "$HOME/.profile"
      ;;
  esac
}

update_shell_path() {
  prefix_bin=$1
  [ "$INSTALL_SCOPE" = "global" ] || return 0
  [ "$UPDATE_PATH" = "1" ] || return 0
  path_contains "$prefix_bin" && return 0
  [ "${HOME:-}" != "" ] || return 0
  [ -d "$HOME" ] || mkdir -p "$HOME" || return 0

  profile=$(shell_profile_path) || return 0
  marker="# >>> Tree Ring Memory PATH >>>"
  if [ -f "$profile" ] && grep -F "$marker" "$profile" >/dev/null 2>&1; then
    PATH_PROFILE_UPDATED=$profile
    return 0
  fi

  quoted_prefix_bin=$(shell_quote "$prefix_bin")
  if {
    printf '\n%s\n' "$marker"
    printf 'TREE_RING_BIN_DIR=%s\n' "$quoted_prefix_bin"
    printf 'case ":$PATH:" in\n'
    printf '  *":$TREE_RING_BIN_DIR:"*) ;;\n'
    printf '  *) export PATH="$TREE_RING_BIN_DIR:$PATH" ;;\n'
    printf 'esac\n'
    printf '# <<< Tree Ring Memory PATH <<<\n'
  } >> "$profile"
  then
    PATH_PROFILE_UPDATED=$profile
  fi
}

intro
require_cargo

PREFIX=$(install_prefix)
BIN="$PREFIX/bin/tree-ring"

install_binary "$PREFIX"
update_shell_path "$PREFIX/bin"

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
  if [ "$PATH_PROFILE_UPDATED" != "" ] && [ "$UPDATE_PATH" = "1" ]; then
    line "Updated shell profile: $PATH_PROFILE_UPDATED"
    line "Open a new terminal, or run this now:"
  else
    line "Add this to your shell profile if tree-ring is not found:"
  fi
  line "  export PATH=\"$PREFIX/bin:\$PATH\""
fi
if [ "$INSTALL_SCOPE" = "project" ]; then
  line "For project-local use:"
  line "  export PATH=\"$PWD/$PREFIX/bin:\$PATH\""
fi
line "Open the TUI:"
line "  $BIN --root $MEMORY_ROOT tui"
