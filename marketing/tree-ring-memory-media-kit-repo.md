# Tree Ring Memory Media Kit Repo

## Target

- Repository: `https://github.com/TerminallyLazy/tree-ring-memory-media-kit`
- Pages URL: `https://terminallylazy.github.io/tree-ring-memory-media-kit/`
- Purpose: clean public media and press kit for launch outreach, creator demos,
  social profiles, newsletter editors, and directory maintainers.
- Commit: `7f67f24459505848b86eec2af36e63e7d20a1a69`
- Validation workflow:
  `https://github.com/TerminallyLazy/tree-ring-memory-media-kit/actions/runs/28947408575`
- Pages deployment:
  `https://github.com/TerminallyLazy/tree-ring-memory-media-kit/actions/runs/28947417271`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4915447730`

## Contents

- `README.md`
- `press.md`
- `social-posts.md`
- `creator-brief.md`
- `docs/index.html`
- `.github/workflows/validate.yml`
- `assets/tree-ring-memory-logo.png`
- `assets/tree-ring-memory-banner.png`
- `assets/open-graph-1200x675.png`
- `assets/social-square-logo-1080x1080.png`
- `assets/social-square-banner-1080x1080.png`
- `assets/x-header-1500x500.png`
- `assets/reddit-link-card-1600x900.png`
- `assets/youtube-thumbnail-1920x1080.png`
- `assets/youtube-channel-banner-2560x1440.png`
- `assets/ratatui-tui-screenshot-1200x675.png`

## Validation

- Local file presence checks passed before commit.
- `git diff --cached --check` passed before commit.
- Public GitHub Actions `Validate media kit` workflow passed.
- GitHub Pages deployment passed and `status` is `built`.
- Repository is public with discovery topics:
  `tree-ring-memory`, `media-kit`, `press-kit`, `agent-memory`, `ai-memory`,
  `local-first`, `launch-assets`, `developer-tools`, `rust-cli`,
  `llm-memory`.
- GitHub Pages configured from `main` `/docs` with HTTPS enforced.

## Notes

The main Tree Ring Memory repo remains the canonical project. This repository
is the shareable campaign surface for press assets and platform-specific launch
copy without exposing the operator CRM in `marketing/`.
