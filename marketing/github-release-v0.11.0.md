# Tree Ring Memory v0.11.0 Launch Preview

Tree Ring Memory is a framework-agnostic, local-first memory lifecycle layer for
AI agents.

Launch page:
https://terminallylazy.github.io/Tree-Ring-Memory/

Feedback issue:
https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26

## Why This Release Exists

Agent memory should age, not pile up. Tree Ring Memory helps agents preserve
useful decisions, warnings, preferences, and evidence without turning memory
into raw transcript storage.

Fresh memory stays detailed. Older learning compresses into rings. Important
failures remain visible as scars. Durable truths become heartwood. Future ideas
stay as seeds.

## What Ships

- Rust-native CLI and crates.
- Local SQLite/FTS storage.
- Explainable recall with ring, scope, confidence, and ranking signals.
- JSONL import/export.
- Audit for memory quality, privacy, and integrity.
- Deterministic consolidation without requiring an LLM.
- Rust-owned maintenance for expiry, secret redaction, and FTS repair.
- Source-linked evaluated outcomes through `tree-ring evidence`.
- DOX and Revolve sync adapters.
- Read-only agent-framework discovery.
- Terminal onboarding and a Ratatui operator console.

## Install

Global user install:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

Project-local install with initialization:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh -s -- --project --init
```

## First Commands

```bash
tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." --event-type lesson --scope project
tree-ring recall "release changes"
tree-ring evidence "The eval passed after the fix." --outcome promoted --evidence-ref evals/run-042
tree-ring audit --audit-type sensitive
tree-ring tui
```

## Release Artifact

This release includes a platform tarball built by `scripts/package-release.sh`
and a SHA-256 checksum file.

The installer can consume prebuilt archives through:

```bash
sh install.sh --archive-url <release-asset-url> --archive-sha256 <sha256>
```

## Feedback Wanted

- Which agent frameworks should get first-class bridge support?
- What should explainable recall show by default?
- Where does the ring model feel too simple or too heavy?
- What privacy or forgetting control is missing?
- What makes the first ten minutes hard?
