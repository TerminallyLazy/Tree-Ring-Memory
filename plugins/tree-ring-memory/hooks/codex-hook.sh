#!/bin/sh
set -eu

if command -v git >/dev/null 2>&1; then
    project_root=$(git rev-parse --show-toplevel 2>/dev/null || true)
    if [ -n "$project_root" ]; then
        cd "$project_root"
    fi
fi

# A linked worktree or unrelated project may not have opted into local memory.
# Existing roots, including dangling symlinks, still reach runtime diagnostics.
if [ ! -e .tree-ring ] && [ ! -L .tree-ring ]; then
    exit 0
fi

# Read only bounded, ordinary Git metadata. Uncertain layouts must not suppress
# plugin dispatch based on a hook definition the host might never load.
read_git_metadata() {
    [ -f "$1" ] && [ ! -L "$1" ] || return 1
    metadata_size=$(wc -c < "$1") || return 1
    [ "$metadata_size" -le 65536 ] || return 1
    metadata_without_nul_size=$(LC_ALL=C tr -d '\000' < "$1" | wc -c) || return 1
    [ "$metadata_size" -eq "$metadata_without_nul_size" ] || return 1
    metadata_value=$(cat "$1") || return 1
    case "$metadata_value" in
        *'
'*) return 1 ;;
    esac
    printf '%s\n' "$metadata_value" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
}

canonical_directory() (
    [ -d "$1" ] && [ ! -L "$1" ] || exit 1
    cd "$1" && pwd -P
)

gitdir_target() {
    pointer=$(read_git_metadata "$1") || return 1
    case "$pointer" in
        gitdir:*) pointer=${pointer#gitdir:} ;;
        *) return 1 ;;
    esac
    pointer=$(printf '%s\n' "$pointer" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')
    [ -n "$pointer" ] || return 1
    case "$pointer" in
        /*) printf '%s\n' "$pointer" ;;
        *) printf '%s/%s\n' "$2" "$pointer" ;;
    esac
}

# Codex keeps normal worktree config local, but substitutes the corresponding
# primary-checkout hook source. Prove the reciprocal linked-worktree layout;
# a common-dir parent alone does not establish an owning primary checkout.
# Return 2 for an ordinary/non-linked checkout, 1 for uncertain metadata.
primary_hook_checkout() (
    checkout=$(pwd -P) || exit 1
    if [ -d .git ] && [ ! -L .git ]; then
        exit 2
    fi
    if [ ! -e .git ] && [ ! -L .git ]; then
        exit 2
    fi
    admin_path=$(gitdir_target "$checkout/.git" "$checkout") || exit 1
    admin=$(canonical_directory "$admin_path") || exit 1
    admin_parent=$(dirname "$admin")
    [ "$(basename "$admin_parent")" = worktrees ] || exit 2
    common=$(dirname "$admin_parent")
    backlink=$(read_git_metadata "$admin/gitdir") || exit 1
    [ -n "$backlink" ] || exit 1
    case "$backlink" in
        /*) ;;
        *) backlink="$admin/$backlink" ;;
    esac
    [ "$(basename "$backlink")" = .git ] || exit 1
    backlink_parent=$(canonical_directory "$(dirname "$backlink")") || exit 1
    [ "$backlink_parent" = "$checkout" ] || exit 1
    common_pointer=$(read_git_metadata "$admin/commondir") || exit 1
    [ -n "$common_pointer" ] || exit 1
    case "$common_pointer" in
        /*) ;;
        *) common_pointer="$admin/$common_pointer" ;;
    esac
    resolved_common=$(canonical_directory "$common_pointer") || exit 1
    [ "$resolved_common" = "$common" ] || exit 1
    # Match Codex's lexical candidate before proving ownership. Canonicalizing
    # an alias of only the worktrees directory could otherwise invent a primary
    # hook source that Codex itself rejects.
    primary=$(dirname "$(dirname "$(dirname "$admin_path")")")
    if [ -d "$primary/.git" ] && [ ! -L "$primary/.git" ]; then
        primary_git=$(canonical_directory "$primary/.git") || exit 1
    else
        primary_pointer=$(gitdir_target "$primary/.git" "$primary") || exit 1
        primary_git=$(canonical_directory "$primary_pointer") || exit 1
    fi
    [ "$primary_git" = "$common" ] || exit 1
    cd "$primary" && pwd -P
)

managed_hook=.codex/hooks.json
primary_checkout=$(primary_hook_checkout 2>/dev/null) && checkout_kind=0 || checkout_kind=$?
case "$checkout_kind" in
    0)
        # No worktree .codex directory means Codex creates no root project layer.
        if [ -d .codex ]; then
            managed_hook="$primary_checkout/.codex/hooks.json"
        else
            managed_hook=
        fi
        ;;
    2) ;;
    *) managed_hook= ;;
esac

# Only the effective project hook owns recall and checkpoints. Never fall back
# to an ignored worktree-local hook when the primary source is absent.
if [ -n "$managed_hook" ] && [ -f "$managed_hook" ] && {
    grep -Fq 'Tree Ring Memory managed lifecycle v2"' "$managed_hook" ||
        grep -Fq 'Tree Ring Memory managed lifecycle v3"' "$managed_hook" ||
        grep -Fq 'Tree Ring Memory managed lifecycle v4"' "$managed_hook"
}; then
    exit 0
fi

tree_ring=tree-ring
if [ -x .tree-ring/bin/tree-ring ]; then
    tree_ring=.tree-ring/bin/tree-ring
fi

exec "$tree_ring" --root .tree-ring integrations hook --harness codex --input-json-stdin
