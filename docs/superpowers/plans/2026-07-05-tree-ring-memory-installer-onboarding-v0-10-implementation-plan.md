# Tree Ring Memory Installer And Onboarding v0.10 Implementation Plan

## Task 1: Add Rust Welcome Command

Files:

- `crates/tree-ring-memory-cli/src/main.rs`
- `crates/tree-ring-memory-cli/src/welcome.rs`

Work:

- Add `tree-ring welcome`.
- Add `--init` and `--no-animation` options.
- Respect global `--root` and `--json`.
- Print concise onboarding commands.
- Initialize SQLite only when `--init` is present.
- Add tests for no-animation output, init behavior, and JSON behavior.

Checks:

```bash
cargo test -p tree-ring-memory-cli welcome
cargo run -q -p tree-ring-memory-cli -- welcome --no-animation
```

## Task 2: Add Installer Script

Files:

- `install.sh`

Work:

- Implement POSIX shell installer.
- Default to global install in `$HOME/.local`.
- Add `--project`, `--init`, `--no-init`, `--no-animation`, `--install-dir`,
  `--repo`, `--ref`, and `--source`.
- Use `cargo install --path` for local source smoke and `cargo install --git`
  for the curl path.
- Run `tree-ring welcome` after successful install unless disabled.
- Give clear recovery messages for missing cargo or failed install.

Checks:

```bash
sh install.sh --help
sh install.sh --source . --install-dir /tmp/tree-ring-install --no-animation --no-init
```

## Task 3: Update Docs And Verify

Files:

- `README.md`
- `docs/architecture/rust-core-status.md`

Work:

- Add one-line install commands.
- Add project-local install and init guidance.
- Add TUI launch commands for installed and source-checkout paths.
- Add installer checks to development commands.

Final checks:

```bash
cargo test
sh install.sh --help
sh install.sh --source . --install-dir /tmp/tree-ring-install --no-animation --no-init
cargo run -q -p tree-ring-memory-cli -- welcome --no-animation
cargo run -q -p tree-ring-memory-cli -- --root /tmp/tree-ring-welcome welcome --init --no-animation
git diff --check
```
