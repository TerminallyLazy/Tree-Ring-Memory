# AINative Ecosystem PR

## Outlet

- Directory: `AINative-Studio/ainative-ecosystem`
- Public PR: https://github.com/AINative-Studio/ainative-ecosystem/pull/21
- Fork: https://github.com/TerminallyLazy/ainative-ecosystem
- Branch: `add/tree-ring-memory`
- Commit: `7ad0056`

## Submission

Added Tree Ring Memory to `data/ecosystem-tools.yaml`:

```yaml
- name: "Tree Ring Memory"
  slug: "tree-ring-memory"
  description: "Local-first memory lifecycle for AI coding agents with recall, redaction, audit, and adapters."
  category: "agent-memory"
  homepage: "https://terminallylazy.github.io/Tree-Ring-Memory/"
  repo: "https://github.com/TerminallyLazy/Tree-Ring-Memory"
  license: "MIT"
  language: "Rust"
  open_source: true
  tags: ["memory", "agent-memory", "local-first", "coding-agents", "rust", "sqlite"]
```

## Fit

AINative accepts AI-native developer tools and has a direct `agent-memory`
category. Tree Ring fits as a free open-source local-first memory lifecycle
framework for AI coding agents.

## Disclosure

The PR body discloses affiliation with Tree Ring Memory.

## Validation

- `python3 scripts/validate.py` passed.
- `python3 scripts/generate-json.py` passed.
- Targeted unique `tree-ring-memory` slug check passed.
- `git diff --check` passed.
- Verified the PR is open, draft, mergeable, and has no reported checks at
  submission time.
