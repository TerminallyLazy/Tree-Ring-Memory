# Tree Ring Memory Agent Plugin

This directory is the repository-distributed Tree Ring Memory plugin for
ChatGPT/Codex and Claude Code. It packages instruction files only; the local
Tree Ring Memory CLI remains the runtime and data owner.

The Codex manifest is version `0.3.2`. The Claude Code manifest is version
`0.3.1`. Both target Tree Ring Memory CLI `0.14.0` or newer and share the same
reviewed wrapper skill.

The package does not run a background service, scrape chats, install hooks, or
ship an MCP server. The active agent decides when a local, source-linked,
privacy-safe memory action is warranted.

## Install Tree Ring Memory

On macOS ARM64:

```bash
brew tap TerminallyLazy/tree-ring
brew install tree-ring
tree-ring --version
```

For other platforms, use the
[canonical installation guide](https://github.com/TerminallyLazy/Tree-Ring-Memory#install).
The plugin must not install or upgrade the CLI, edit shell configuration, or
claim a memory action ran without explicit permission and observed output.

## Install In ChatGPT And Codex

Add this repository as a marketplace:

```bash
codex plugin marketplace add TerminallyLazy/Tree-Ring-Memory
```

Restart the ChatGPT desktop app, open the Plugins Directory, select the Tree
Ring Memory marketplace, and install Tree Ring Memory. When the repository is
already open in Work mode or Codex, `.agents/plugins/marketplace.json` also
exposes the repo-scoped package.

OpenAI ZIP submission remains a separate public-directory channel. Its
skills-only package intentionally includes the logo and composer icon but no
`interface.screenshots` field.

## Install In Claude Code

From Claude Code:

```text
/plugin marketplace add TerminallyLazy/Tree-Ring-Memory
/plugin install tree-ring-memory@tree-ring-memory
```

The package adds the `tree-ring-memory` skill and these commands:

- `/tree-ring-memory:tree-ring-recall`
- `/tree-ring-memory:tree-ring-capture`
- `/tree-ring-memory:tree-ring-audit`
- `/tree-ring-memory:tree-ring-status`
- `/tree-ring-memory:tree-ring-dox-sync`
- `/tree-ring-memory:tree-ring-certify`

## DOX And Certification Boundaries

The wrapper skill includes a DOX contract flow. It reads the applicable live
`AGENTS.md` chain, previews `tree-ring dox sync` before persistence, keeps
source contracts authoritative, and never rewrites them.

Installed runtimes can produce bounded evidence with
`tree-ring integrations certify` and `tree-ring recall-quality`. The full
`scripts/certify-tree-ring.sh` release suite depends on the Rust workspace,
fixtures, installer, and build tools in a complete Tree Ring Memory source
checkout. It is intentionally not copied into this plugin. The TUI and plugin
may point to that command, but neither silently runs it nor turns its absence
into a false certification claim.

## Security

This plugin ships instructions and local assets only. It includes no remote MCP
server, webhooks, analytics, credentials, or networked runtime code. See
[PRIVACY.md](PRIVACY.md), [SECURITY.md](SECURITY.md), and [TERMS.md](TERMS.md).
