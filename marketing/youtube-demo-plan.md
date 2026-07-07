# Tree Ring Memory YouTube Demo Plan

Target length: 3-5 minutes.

## Title

Tree Ring Memory: Local-first memory lifecycle for AI agents

## Thumbnail

Use `marketing/assets/youtube-thumbnail-1920x1080.png`.

## Description

Use the YouTube description in `marketing/launch-kit.md`.

## Shot List

1. Show the banner and one-sentence premise.
2. Show the install command.
3. Run `tree-ring init`.
4. Store one lesson with `tree-ring remember`.
5. Recall with `tree-ring recall`.
6. Record evaluated evidence with `tree-ring evidence`.
7. Show privacy posture with `tree-ring audit --audit-type sensitive`.
8. Open `tree-ring tui`.
9. End on GitHub issue `#26` for feedback.

## Terminal Script

```bash
tmpdir="$(mktemp -d)"
cd "$tmpdir"

tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." \
  --event-type lesson \
  --scope project \
  --project demo-agent \
  --tag release \
  --tag workflow
tree-ring recall "release changes" --project demo-agent
tree-ring evidence "A promoted evaluation fixed stale recall in the demo agent." \
  --outcome promoted \
  --evidence-ref evals/demo-agent/run-042 \
  --project demo-agent \
  --score 0.91
tree-ring audit --audit-type sensitive
tree-ring tui
```

## Voiceover Beats

- Agent memory should not be raw transcript storage.
- Tree Ring Memory makes memory age: cambium, rings, scars, heartwood, seeds.
- Recall should be scoped and explainable.
- Evaluated outcomes belong in memory with source references.
- Forgetting and redaction are part of the product, not cleanup chores.
- The current runtime is local-first and Rust-native.
- Feedback needed: adapters, recall explanations, first-run friction.

## Recording Notes

- Use a clean temporary directory.
- Use a large terminal font.
- Keep the banner visible in the thumbnail, not necessarily in the terminal.
- Do not show private paths, usernames, tokens, shell history, or real project
  memory.
- If the TUI is too visually busy for the first video, use a short clip only
  and keep the explanation in voiceover.
