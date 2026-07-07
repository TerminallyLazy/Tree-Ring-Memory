# Tree Ring Memory Marketing

Launch and outreach materials for Tree Ring Memory.

## Start Here

- `account-setup.md`: account identity, handles, bios, signup URLs, secure
  setup checklist, and profile asset mapping.
- `launch-kit.md`: channel-specific launch copy for GitHub, Hacker News,
  Reddit, X, YouTube, Product Hunt, LinkedIn, developer blogs, newsletters, and
  directories.
- `channel-playbook.md`: execution-ready platform playbook with current
  platform guardrails and post order.
- `reply-bank.md`: response bank for HN, Reddit, Discussions, X, YouTube, and
  newsletter follow-up.
- `outreach-crm.csv`: outlet-by-outlet execution CRM with next action and
  asset mapping.
- `launch-calendar.md`: five-day launch sequence and success signals.
- `ad-directions.md`: deterministic campaign image directions based on the
  existing Tree Ring Memory visual system.
- `scripts/build-campaign-cards.py`: reproducible PIL builder for the generated
  Homebrew, Rust article, TWiR, and square social cards.
- `outreach-queue.md`: execution tracker for live, pending, and held launch
  surfaces.
- `newsletter-pitches.md`: short pitches for newsletters, directories, and
  community maintainers.
- `terminal-trove-submission.md`: Terminal Trove submission packet and email
  draft fields.
- `youtube-demo-plan.md`: shot list, terminal script, and production notes for
  the first demo video.
- `youtube/`: upload-ready YouTube source package with title, description,
  captions, tags, voiceover, slide renderer, and reproducible MP4 build script.
- `submission-ledger.csv`: platform-by-platform status tracker for external
  outlets.
- Pages discovery files: `docs/robots.txt`, `docs/sitemap.xml`, `docs/llms.txt`,
  and `docs/press-kit.md`.
- `github-feedback-issue.md`: body used for the live launch feedback issue.
- `social-profiles.json`: structured account registry for public profile URLs
  as accounts are created.
- `assets/`: resized social, header, thumbnail, and channel-banner images
  derived from the source brand assets.

## Generated Campaign Cards

Regenerate the checked-in launch cards with:

```bash
python3 marketing/scripts/build-campaign-cards.py
```

- `marketing/assets/homebrew-install-card-1200x675.png`
- `marketing/assets/rust-article-card-1200x675.png`
- `marketing/assets/twir-submission-card-1200x675.png`
- `marketing/assets/not-transcript-dump-card-1080x1080.png`
- `marketing/assets/terminal-trove-preview-1200x675.png`

## Live Public Surfaces

- Repository: `https://github.com/TerminallyLazy/Tree-Ring-Memory`
- Launch page: `https://terminallylazy.github.io/Tree-Ring-Memory/`
- Launch release: `https://github.com/TerminallyLazy/Tree-Ring-Memory/releases/tag/v0.11.0`
- Launch discussion: `https://github.com/TerminallyLazy/Tree-Ring-Memory/discussions/27`
- Homebrew tap: `https://github.com/TerminallyLazy/homebrew-tree-ring`
- Launch feedback issue: `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26`
- Text launch page: `docs/launch/tree-ring-memory-framework.md`
- Rust-native CLI article: `https://terminallylazy.github.io/Tree-Ring-Memory/launch/rust-native-agent-memory-cli.md`
- Awesome Local-First PR: `https://github.com/alexanderop/awesome-local-first/pull/42`
- Awesome AI SDKs PR: `https://github.com/e2b-dev/awesome-ai-sdks/pull/268`
- Awesome Command Line Tools PR: `https://github.com/ad-si/awesome-command-line-tools/pull/2`
- Awesome Agent Memory PR: `https://github.com/TeleAI-UAGI/Awesome-Agent-Memory/pull/53`
- Awesome TUIs PR: `https://github.com/rothgar/awesome-tuis/pull/751`
- Awesome Ratatui PR: `https://github.com/ratatui/awesome-ratatui/pull/361`
- Press kit: `https://terminallylazy.github.io/Tree-Ring-Memory/press-kit.md`
- LLM summary: `https://terminallylazy.github.io/Tree-Ring-Memory/llms.txt`
- Atom feed: `https://terminallylazy.github.io/Tree-Ring-Memory/feed.xml`

## Drafted For Owner Review

- Terminal Trove email draft: prepared for `curator@terminaltrove.com` with
  `marketing/assets/terminal-trove-preview-1200x675.png` attached. Review and
  send from Gmail, or paste the fields from `terminal-trove-submission.md` into
  the Terminal Trove form at `https://terminaltrove.com/post/`.

## Account-Creation Boundary

Account creation for X, Reddit, Hacker News, YouTube, Bluesky, Mastodon,
LinkedIn, Product Hunt, Dev.to, Hashnode, Medium, Substack, and Discord requires
owner-controlled email, password manager, phone/email verification, CAPTCHA, or
SSO in the browser. Do not store account secrets, recovery codes, tokens, or
private contact details in this repo.

After each account exists, add only the public profile URL and status to
`social-profiles.json`.
