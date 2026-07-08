# JSONbored HeyClaude Awesome Claude PR

## Target

- Repository: `https://github.com/JSONbored/awesome-claude`
- Directory: HeyClaude / Awesome Claude
- PR: `https://github.com/JSONbored/awesome-claude/pull/4618`
- Fork: `https://github.com/TerminallyLazy/awesome-claude`
- Branch: `add-tree-ring-memory-skill`
- Commit: `cd0a082d8919f53b104ce12f267c0e634aa746ef`
- Merge commit: `5b1f228af735168605018d81e4449a64995b9ef1`
- Upstream content:
  `https://github.com/JSONbored/awesome-claude/blob/main/content/skills/tree-ring-memory.mdx`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4914051309`
- Current state: merged on 2026-07-08T10:56:32Z; HeyClaude PR Validation,
  coverage, and Superagent checks passed.

## What Changed

- Added `content/skills/tree-ring-memory.mdx` as a source-backed HeyClaude
  skills entry.
- Positioned Tree Ring Memory as a framework-agnostic memory lifecycle skill
  plus Rust-native CLI.
- Covered explicit recall and capture, evidence-backed outcomes, local
  SQLite/FTS storage, forgetting/redaction/audit, deterministic consolidation,
  DOX/Revolve adapter previews, and portable agent skill guidance.
- Used a Cargo git install command in executable snippets so the direct
  submission passes HeyClaude's unsafe install-pipeline policy.

## Validation

- `pnpm validate:content:strict`
- `pnpm validate:content:strict -- --category skills`
- `pnpm audit:content -- --category skills`
- `node scripts/ci/validate-content-policy.mjs --repo-root . --files-json /tmp/heyclaude-tree-ring-changed-files.json`
- `git diff --check`
- `git diff --check origin/main...HEAD`

## Notes

- The first CI attempt failed because the listing used the canonical
  `curl ... | sh` installer snippet, which HeyClaude classifies as an unsafe
  install pipeline for direct submissions.
- The PR was amended to use:
  `cargo install --git https://github.com/TerminallyLazy/Tree-Ring-Memory tree-ring-memory-cli --locked`.
- The corrected PR passed HeyClaude's direct submission content gate, registry
  gate, required PR gate, coverage, and Superagent checks on commit
  `cd0a082d8919f53b104ce12f267c0e634aa746ef`.
- Public site check after merge returned `https://heyclau.de/browse` for the
  guessed skill route and did not yet show Tree Ring text in fetched browse
  HTML; monitor the next deployment/index refresh before marking the site page
  live.
- External social/media account creation remains owner-auth gated for
  email/phone/CAPTCHA/SSO and terms acceptance; this outlet did not require a
  new account beyond the authenticated GitHub owner account.
