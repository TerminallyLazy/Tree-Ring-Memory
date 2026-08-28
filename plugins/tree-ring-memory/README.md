# Tree Ring Memory Agent Plugin

This directory is the repository-distributed Tree Ring Memory plugin for
ChatGPT/Codex and Claude Code. It packages reviewed instructions plus thin
lifecycle-hook registrations; the local Tree Ring Memory CLI remains the
runtime and data owner.

The Codex manifest is version `0.3.4`. The Claude Code manifest is version
`0.3.3`. Both target Tree Ring Memory CLI `0.15.0` or newer and share the same
reviewed wrapper skill.

The repository plugin registers exactly `SessionStart`, `SubagentStart`,
`Stop`, and `SubagentStop`. Each hook forwards its event JSON directly to the
local CLI and waits synchronously for at most 10 seconds. It does not register
prompt, tool, compaction, or `SessionEnd` hooks; run a background service;
scrape chats; or ship an MCP server.

Session start covers startup, resume, and compaction rehydration when the host
reports those sources. Subagent start gives each worker an independent,
receipt-backed preflight. Codex requires review and trust of the installed hook
definition before it runs. Claude Code loads the hook with the enabled plugin.

Stop and subagent-stop enforce one agent-mediated memory checkpoint. The
lifecycle parser uses only stable harness identity and project fields; it never
inspects or persists `transcript_path`, `last_assistant_message`, prompts, or
transcript content. The checkpoint asks the active agent to evaluate its
already-grounded work. If and only if that evaluation yields a concise,
durable, normal-sensitivity candidate, the agent automatically runs the exact
strict `tree-ring capture` command template returned by the lifecycle handler.
Strict capture fixes agent scope, requires identity and provenance, adds an
automatic-capture tag, and rejects sensitive candidates. No candidate means no
memory write. This is one bounded checkpoint, not a recorder or automatic
summary of every turn.

The hook wrapper resolves the Git project root when available, prefers that
project's `.tree-ring/bin/tree-ring`, and otherwise uses `tree-ring` from
`PATH`. It then invokes the shared lifecycle entry point with the project-local
`.tree-ring` root. An unavailable or incompatible CLI is not active-harness
proof and cannot be reported as a successful checkpoint or capture.

When project activation has already installed the managed lifecycle definition
in `.codex/hooks.json` or `.claude/settings.json`, that project definition owns
recall and stop checkpoints. The marketplace wrapper detects the exact managed
marker and exits without invoking the CLI, preventing duplicate context,
receipts, checkpoint continuations, or capture attempts when the host merges
project and plugin hooks.

## Install Or Update Tree Ring Memory

The safe default is a verified project-local install. From the actual project
root, after the user has authorized Tree Ring setup:

Download the official version-pinned `v0.15.0/install.sh` to a temporary file,
verify its SHA-256 is
`ef0d5eb8f09cbe2e4c3abe80ee9a98a56759c89ad4ddd103d6c68314cd653ade`, inspect
it, then run these commands from the project root:

```bash
sh <verified-installer-path> --project --init --release latest --no-animation
.tree-ring/bin/tree-ring --root .tree-ring integrations status --verbose
```

Do not pipe a network response directly to a shell.

The installer downloads the official platform release and verifies its SHA-256
asset. A global Homebrew install remains available on macOS ARM64 through
`brew install tree-ring`. Agents may proceed when the user's request already
authorizes setup; otherwise they must explain the operation and obtain
permission before downloading or installing software. They must never
initialize inside the plugin cache, change global scope, edit shell
configuration, or claim a memory action ran without the required authorization
and observed output.

Use `tree-ring update --check` to check without changing files. With update
authorization, `tree-ring update` preserves the active project-local, direct,
or Homebrew install scope. Afterward, rerun `tree-ring --root .tree-ring init`
from each project root so managed guidance is refreshed without overwriting
custom files. See the
[canonical installation guide](https://github.com/TerminallyLazy/Tree-Ring-Memory#install)
for older CLIs and additional platforms.

## Install In ChatGPT And Codex

Add this repository as a marketplace:

```bash
codex plugin marketplace add TerminallyLazy/Tree-Ring-Memory
```

Restart the ChatGPT desktop app, open the Plugins Directory, select the Tree
Ring Memory marketplace, and install Tree Ring Memory. When the repository is
already open in Work mode or Codex, `.agents/plugins/marketplace.json` also
exposes the repo-scoped package.

OpenAI ZIP submission remains a separate public-directory channel. Build that
skills-only artifact explicitly instead of uploading the repository plugin:

```bash
python3 plugins/tree-ring-memory/packaging/build-codex-skills-only.py \
  tree-ring-memory-codex-skills-only.zip
```

The generated ZIP has one `tree-ring-memory/` package root. It includes the
skill, legal notices, logo, composer icon, and a dedicated skills-only manifest;
it excludes lifecycle hooks, Claude commands and metadata, MCP servers, apps,
and `interface.screenshots`. The repository plugin and public upload artifact
are intentionally separate validation profiles.

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
- `/tree-ring-memory:tree-ring-update`

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

This plugin ships instructions, local assets, and local lifecycle-hook
registrations. It includes no remote MCP server, webhooks, analytics,
credentials, or networked runtime code. See
[PRIVACY.md](PRIVACY.md), [SECURITY.md](SECURITY.md), and [TERMS.md](TERMS.md).
