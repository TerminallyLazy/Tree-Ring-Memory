Tree Ring Memory is a framework-agnostic, local-first memory lifecycle layer for AI agents.

Website:
https://terminallylazy.github.io/Tree-Ring-Memory/

Repository:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Launch feedback:
https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26

It helps agents remember useful decisions, warnings, preferences, and evaluated lessons without turning memory into transcript dumps.

In this demo:

- why agent memory needs a lifecycle
- the ring model: cambium, outer, inner, heartwood, scars, and seeds
- what ships in the Rust-native public runtime
- local install and first commands
- explicit privacy boundary
- where to leave launch feedback

Install:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

Try:

```bash
tree-ring init
tree-ring remember "Use project-scoped recall before risky changes." --event-type lesson --scope project
tree-ring recall "risky changes"
tree-ring evidence "The eval passed after the fix." --outcome promoted --evidence-ref evals/run-042
tree-ring tui
```

Chapters:

00:00 Why agent memory needs a lifecycle
00:18 The tree-ring model
00:36 What ships now
00:53 Local install and first commands
01:14 Privacy boundary
01:36 Feedback wanted

Tree Ring Memory is protocol-preview software. The launch question is practical: what should a portable, local-first memory layer get right before deeper framework bridges land?
