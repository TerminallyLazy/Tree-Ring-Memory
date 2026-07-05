# Historical Planning Records

This directory preserves earlier Tree Ring Memory planning records. They are
useful context for how the project moved from a prototype into a Rust-native
framework, but they are not the current implementation contract.

Current source of truth:

- `README.md`
- `docs/architecture/rust-core-status.md`
- `docs/architecture/rust-core-roadmap.md`
- `docs/protocol/memory-event.md`
- `skills/tree-ring-memory/SKILL.md`

Some archived plans and specs mention a Python prototype, PyO3 bindings, pytest,
or transitional compatibility surfaces. Those references are historical. The
canonical runtime is now Rust-native: crates plus the `tree-ring` CLI own
storage, recall, import/export, audit, consolidation, maintenance, source
adapters, framework discovery, and the terminal UI.
