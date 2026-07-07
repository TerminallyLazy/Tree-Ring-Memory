# Terminal Trove Submission

Source: <https://terminaltrove.com/post/>

Terminal Trove accepts tool submissions by form or email and asks for a preview
image, install commands, categories, and confirmation that the tool is not
already listed.

Current status: submission email sent to `curator@terminaltrove.com` with the
preview image attached. Await curator response or public listing.

## Criteria Check

- Cross platform: current public binary is macOS ARM64; source builds with Cargo.
- Standalone binaries: v0.11.0 release ships a macOS ARM64 tarball.
- Preview image: `marketing/assets/terminal-trove-preview-1200x675.png`.
- Existing listing: searched Terminal Trove for `Tree Ring Memory` and
  `tree-ring memory`; no existing listing was found.

## Form Fields

Name:
Tree Ring Memory

URL:
terminallylazy.github.io/Tree-Ring-Memory/

Tagline:
Local-first memory lifecycle CLI for AI agents.

Description:
Tree Ring Memory helps AI agents keep useful lessons, warnings, preferences,
and evidence without turning memory into transcript dumps. It stores memory
locally, supports explainable SQLite/FTS recall, and gives operators audit,
redaction, forgetting, consolidation, and a Ratatui TUI.

Standout features:
Rust-native CLI with local SQLite/FTS recall; explicit durable writes instead of
background transcript capture; audit, redaction, forgetting, deterministic
consolidation, and terminal TUI workflows.

Other notable features:
Homebrew tap for macOS ARM64, one-line installer, JSONL import/export,
DOX/Revolve sync adapters, agent-framework discovery, and local-first defaults.

Who is this for / when to use it:
Developers and AI-agent operators who want persistent, auditable, local memory
for coding agents, private agent workflows, and framework-agnostic experiments.

Primary language:
rust

License:
mit

Categories:
macos, cli, ai, gpt, rust, sqlite, ratatui, tui, productivity, utilities

Install instructions:

```bash
brew tap TerminallyLazy/tree-ring
brew install tree-ring
```

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

Author:
yes

Email route:
curator@terminaltrove.com
