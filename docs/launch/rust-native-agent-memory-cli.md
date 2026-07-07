# Building A Rust-Native Memory Lifecycle CLI For AI Agents

Tree Ring Memory is a local-first memory lifecycle layer for AI agents. The
current public runtime is Rust-native: a workspace of crates for protocol
models, SQLite/FTS storage, recall, audit, consolidation, maintenance, and a
terminal operator console.

The project started from a product problem rather than a database problem:
agent memory tends to either forget the useful things or preserve too much raw
transcript. Tree Ring Memory treats memory as something that ages.

## The Workspace Shape

The repository is organized as three Rust crates:

- `tree-ring-memory-core`: event models, validation, sensitivity checks, recall
  scoring, and ring concepts.
- `tree-ring-memory-sqlite`: schema-compatible SQLite/FTS storage and recall
  filtering.
- `tree-ring-memory-cli`: the `tree-ring` command-line interface and Ratatui
  terminal console.

That split keeps the protocol and scoring logic out of the CLI, keeps storage
behind a small boundary, and leaves the command surface thin enough to inspect.

## Why Rust For This Layer?

Agent memory sits close to local files, shell workflows, and editor/terminal
automation. For this project, Rust is useful for a few pragmatic reasons:

- the public runtime can be a small local binary instead of a hosted service;
- storage and recall behavior can stay deterministic and testable;
- the CLI can own privacy-sensitive operations such as audit, redaction, and
  deletion without hidden network behavior;
- the same core models can be reused by future adapters without making every
  integration own the memory lifecycle rules.

The goal is not to make another agent framework. The goal is to give existing
agent workflows a portable memory layer they can call explicitly.

## Why SQLite And FTS?

Tree Ring Memory is local-first by default. Durable memory lives in a local
SQLite store with FTS recall. That makes the first version boring in the right
ways:

- no hosted memory service is required;
- the memory root can live with a project;
- recall remains inspectable;
- import/export can use JSONL;
- audit and maintenance can operate over local records;
- deletion and redaction can be first-class commands.

Vector search can still be an adapter later, but it does not replace lifecycle
policy. Tree Ring Memory cares about what gets written, how it is scoped, how
it ages, how recall is explained, and how memory is intentionally forgotten.

## The Ring Model

The ring names are intentionally plain:

- `cambium`: fresh active context.
- `outer`: recent summarized learning.
- `inner`: older compressed learning.
- `heartwood`: durable high-confidence truths.
- `scar`: important failures, regressions, and warnings.
- `seed`: unresolved ideas and hypotheses.

This is not just labeling. The ring gives recall and maintenance a policy
surface. A failure that should prevent repeated mistakes can remain visible as
a scar. An evaluated outcome can be promoted into heartwood. A speculative
follow-up can stay a seed until it earns more confidence.

## Explicit Writes Instead Of Transcript Capture

The privacy boundary is central to the design. Tree Ring Memory does not scrape
terminal output, record every prompt, or turn an entire chat transcript into
durable memory.

Durable writes are explicit:

```bash
tree-ring remember "Use project-scoped recall before risky release changes." \
  --event-type lesson \
  --scope project \
  --project example-service \
  --tag release

tree-ring evidence "A test run proved the new workflow fixed stale recall." \
  --outcome promoted \
  --evidence-ref evals/recall/run-042
```

That constraint keeps memory smaller, easier to audit, and easier to explain to
future agents.

## Recall Should Explain Itself

A recall result should not feel magical. Tree Ring Memory recall is designed to
carry inspectable signals such as ring, scope, confidence, and ranking factors.

The command path is deliberately simple:

```bash
tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." \
  --event-type lesson \
  --scope project
tree-ring recall "release changes"
```

That is also why the CLI includes audit and maintenance commands. Useful memory
needs cleanup tools, not only write and search.

## Terminal UI As Operator Surface

The `tree-ring tui` console exists for a different use case than the one-shot
CLI commands. When memory starts to become operational infrastructure, people
need to inspect records, run recall, export, consolidate, and review state from
one place.

Ratatui fits that job because the first operator surface should live where many
agent workflows already live: in the terminal.

## What Is Still Open

Tree Ring Memory is in protocol-preview status. The current Rust runtime is
usable, but the adapter contract should be shaped by people building real agent
workflows before it is treated as stable.

The most useful feedback right now:

- Which agent frameworks should get first-class bridge support?
- What should explainable recall show by default?
- Where does the ring model feel too simple or too heavy?
- What privacy or forgetting control is missing?
- What makes the first ten minutes hard?

Try the launch preview:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring tui
```

Links:

- Launch page: <https://terminallylazy.github.io/Tree-Ring-Memory/>
- Release: <https://github.com/TerminallyLazy/Tree-Ring-Memory/releases/tag/v0.11.0>
- Repository: <https://github.com/TerminallyLazy/Tree-Ring-Memory>
- Discussion: <https://github.com/TerminallyLazy/Tree-Ring-Memory/discussions/27>
- Feedback issue: <https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26>

Disclosure: this launch article was prepared with AI-assisted drafting from
the checked-in repository, release, and marketing materials.
