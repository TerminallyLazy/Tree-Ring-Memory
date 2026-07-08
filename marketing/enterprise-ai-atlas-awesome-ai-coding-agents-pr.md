# Enterprise AI Atlas Awesome AI Coding Agents PR

Submission date: 2026-07-07

## Target

- Directory: https://github.com/Enterprise-AI-Atlas/awesome-ai-coding-agents
- Pull request: https://github.com/Enterprise-AI-Atlas/awesome-ai-coding-agents/pull/3
- Fork branch: https://github.com/TerminallyLazy/awesome-ai-coding-agents/tree/add-tree-ring-memory
- Evidence comments:
  - https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4909290440
  - https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911027090

## Submitted Entry

Placement: `Open-Source Frameworks`

```markdown
- **[Tree Ring Memory](https://github.com/TerminallyLazy/Tree-Ring-Memory)** `Community` - Local-first memory lifecycle framework for coding agents that need project-scoped recall, consolidation, redaction, and export.
  - Install: `curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh`
```

## Fit Rationale

Enterprise AI Atlas accepts public, documented, actively maintained resources
for coding agents and agentic software engineering. Tree Ring Memory is framed
as a local-first memory lifecycle framework for coding-agent workflows, not as a
standalone coding agent.

## Validation

- `git diff --check`: passed.
- `./scripts/validate-links.sh`: direct macOS run failed because `/bin/bash`
  lacks Bash 4 `mapfile`.
- `./scripts/validate-links.sh` semantics with a Bash 3-compatible `mapfile`
  shim: passed.
- GitHub PR state at submission: open draft, mergeable, no status checks
  reported.
- Follow-up on 2026-07-08: marked PR ready for review; verified open,
  non-draft, mergeable, and no status checks reported.
