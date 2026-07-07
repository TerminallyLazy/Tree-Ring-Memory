# Tree Ring Memory Press Kit

Tree Ring Memory is a framework-agnostic, local-first memory lifecycle layer for
AI agents.

## Short Description

Tree Ring Memory helps agents remember useful decisions, warnings, preferences,
and evidence without turning memory into transcript dumps.

## Long Description

Tree Ring Memory gives AI agent memory a lifecycle: fresh work stays detailed,
older learning compresses into rings, important failures remain visible as
scars, durable truths become heartwood, and future ideas stay as seeds. The
current public runtime is Rust-native and local-first, with a CLI, SQLite/FTS
storage, explainable recall, audit, deterministic consolidation, forgetting,
DOX/Revolve adapters, framework discovery, and a terminal TUI.

## Key Facts

- Product: Tree Ring Memory
- Category: AI agents, developer tools, local-first software, Rust CLI
- License: MIT
- Status: protocol-preview
- Current version: 0.11.0
- Website: <https://terminallylazy.github.io/Tree-Ring-Memory/>
- Repository: <https://github.com/TerminallyLazy/Tree-Ring-Memory>
- Feedback: <https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26>

## Primary Message

Agent memory should age, not pile up.

## Proof Points

- Local-first by default; no required hosted service.
- Rust-native CLI and crates.
- SQLite/FTS storage and explainable recall.
- First-class forgetting, redaction, supersession, audit, and maintenance.
- Deterministic consolidation without requiring an LLM.
- Source-linked evaluated outcomes through `tree-ring evidence`.
- DOX and Revolve sync adapters.
- Terminal onboarding and Ratatui operator console.

## Launch Copy

```text
Tree Ring Memory is a framework-agnostic, local-first memory lifecycle layer for
AI agents. It helps agents remember useful decisions, warnings, preferences,
and evidence without becoming a transcript dump. Fresh memory stays detailed,
older learning compresses into rings, important failures become scars, durable
truths become heartwood, and future ideas stay as seeds.
```

## Images

- Open Graph card: <https://terminallylazy.github.io/Tree-Ring-Memory/assets/tree-ring-memory-og.png>
- Hero image: <https://terminallylazy.github.io/Tree-Ring-Memory/assets/tree-ring-memory-hero.png>
- Icon: <https://terminallylazy.github.io/Tree-Ring-Memory/assets/tree-ring-memory-icon.png>
- Repository banner: <https://github.com/TerminallyLazy/Tree-Ring-Memory/blob/main/assets/tree-ring-memory-banner.png>
- Repository logo: <https://github.com/TerminallyLazy/Tree-Ring-Memory/blob/main/assets/tree-ring-memory-logo.png>

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

## First Commands

```bash
tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." --event-type lesson --scope project
tree-ring recall "release changes"
tree-ring evidence "The eval passed after the fix." --outcome promoted --evidence-ref evals/run-042
tree-ring tui
```

## Feedback Questions

- Which agent frameworks should get first-class bridge support?
- What should explainable recall show by default?
- Where does the ring model feel too simple or too heavy?
- What privacy or forgetting control is missing?
- What makes the first ten minutes hard?
