# Tree Ring Memory Framework

![Tree Ring Memory banner](../../assets/tree-ring-memory-banner.png)

Tree Ring Memory is a framework-agnostic memory lifecycle layer for AI agents.

It helps agents remember useful decisions, warnings, preferences, and evidence
without turning memory into a transcript dump. Fresh memory stays detailed,
older memory compresses into rings, important scars remain visible, and durable
truths become heartwood.

## Why It Exists

Most agent memory systems fail in one of two ways:

- they forget the hard-earned lessons between runs;
- they preserve too much raw transcript and call it memory.

Tree Ring Memory treats memory as something with a lifecycle: capture,
scope, recall, audit, consolidate, forget, and supersede.

## What Ships Now

- Rust-native CLI and crates.
- Local SQLite/FTS storage.
- Explainable recall with ring, scope, confidence, and ranking signals.
- Explicit forgetting, redaction, supersession, audit, and maintenance.
- Deterministic consolidation without requiring an LLM.
- Source-linked evaluated outcomes through `tree-ring evidence`.
- DOX and Revolve sync adapters.
- Read-only agent-framework discovery.
- Terminal onboarding and a Ratatui operator console.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

Then try:

```bash
tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." --event-type lesson --scope project
tree-ring recall "release changes"
tree-ring evidence "The eval passed after the fix." --outcome promoted --evidence-ref evals/run-042
tree-ring tui
```

## The Ring Model

- `cambium`: fresh active context.
- `outer`: recent summarized learning.
- `inner`: older compressed learning.
- `heartwood`: durable high-confidence truths.
- `scar`: important failures, regressions, and warnings.
- `seed`: unresolved future ideas and hypotheses.

The point is not to store more. The point is to keep memory useful as it ages.

## Privacy Boundary

Tree Ring Memory is local-first by default. It does not scrape transcripts, run
a hidden recorder, or turn terminal event pulses into durable memory.

Durable writes are explicit: `remember`, `evidence`, `import`, consolidation,
maintenance, TUI action, or deliberate agent action.

## Feedback Wanted

The project is in protocol-preview status. Useful launch feedback:

- Which agent frameworks should get first-class bridge support?
- What should explainable recall show by default?
- Where does the ring model feel too simple or too heavy?
- What privacy or forgetting control is missing?
- What makes the first 10 minutes hard?

Leave feedback in the launch issue:

<https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26>

Repository:

<https://github.com/TerminallyLazy/Tree-Ring-Memory>
