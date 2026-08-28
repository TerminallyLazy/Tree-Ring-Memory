#!/bin/sh
set -eu

if command -v git >/dev/null 2>&1; then
    project_root=$(git rev-parse --show-toplevel 2>/dev/null || true)
    if [ -n "$project_root" ]; then
        cd "$project_root"
    fi
fi

# Project activation owns lifecycle recall and checkpoints when its managed hook
# is present. The marketplace hook stands down to prevent duplicate handling.
if [ -f .claude/settings.json ] && {
    grep -Fq 'Tree Ring Memory managed lifecycle v2"' .claude/settings.json ||
        grep -Fq 'Tree Ring Memory managed lifecycle v3"' .claude/settings.json
}; then
    exit 0
fi

tree_ring=tree-ring
if [ -x .tree-ring/bin/tree-ring ]; then
    tree_ring=.tree-ring/bin/tree-ring
fi

exec "$tree_ring" --root .tree-ring integrations hook --harness claude-code --input-json-stdin
