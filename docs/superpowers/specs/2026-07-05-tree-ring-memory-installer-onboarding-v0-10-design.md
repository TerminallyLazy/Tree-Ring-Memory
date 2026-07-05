# Tree Ring Memory Installer And Onboarding v0.10 Design

v0.10 adds a one-line installer and a terminal onboarding path that make Tree
Ring Memory easy to try globally or inside a project.

## Intent

Tree Ring Memory should feel approachable without hiding that it is a serious
local memory tool. The install flow should satisfy the basics first: it works,
handles errors clearly, and leaves the user with obvious next commands. Delight
comes from a short animated ASCII ring moment and useful guidance, not from a
blocking wizard.

## Goals

- Provide a curl-friendly `install.sh`.
- Support global install into `$HOME/.local/bin` by default.
- Support project-local install into `.tree-ring/bin` with `--project`.
- Support optional project initialization with `--init`.
- Add a Rust-native `tree-ring welcome` command with terminal-safe ASCII rings.
- Use animation only when stdout is a TTY and the user has not disabled it.
- Keep non-interactive/scripted output deterministic with `--no-animation`.
- Document how to install, initialize, and open the TUI.

## Non-Goals

- Do not require a package manager beyond an existing Rust toolchain.
- Do not add shell dependencies beyond POSIX `sh` plus `cargo`.
- Do not create cloud accounts or remote sync.
- Do not initialize a project unless the user asks through `--init`.

## Installer UX

Default:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

Project-local:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh -s -- --project --init
```

Emotional arc:

- First encounter: clear retro tree-ring identity.
- Setup: explain where the binary will go before building.
- First success: confirm install and show the next three useful commands.
- Recovery: explain missing Rust/cargo with a concrete fix.

## Acceptance Criteria

1. `install.sh --help` documents global, project, init, no-animation, local
   source, and install-dir options.
2. `install.sh --source . --install-dir <tmp> --no-animation --no-init` installs
   a working `tree-ring` binary.
3. `tree-ring welcome --no-animation` prints onboarding guidance without
   opening SQLite.
4. `tree-ring welcome --init --no-animation` initializes the configured root.
5. `tree-ring welcome --json --init` emits a structured status payload.
6. README includes one-line install, project-local install, and TUI launch
   instructions.
7. `cargo test` passes.
8. `git diff --check` passes.
