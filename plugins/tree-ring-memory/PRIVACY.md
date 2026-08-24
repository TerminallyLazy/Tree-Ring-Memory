# Tree Ring Memory Plugin Privacy Notice

Effective August 23, 2026

The Tree Ring Memory plugin is an instruction package for AI coding agents. It
does not operate a hosted service, create a user account, collect analytics,
send telemetry, or include a remote MCP server.

When an agent runs the separately installed Tree Ring Memory CLI, the CLI stores
the memory content the user chooses in a local SQLite database under the
configured Tree Ring root. The project does not receive that database or its
contents. Data leaves the local environment only when the user or another tool
explicitly exports, syncs, publishes, or otherwise transmits it.

The plugin instructs agents to avoid transcripts, credentials, secrets, private
keys, raw chain-of-thought, and unnecessary sensitive personal data. It also
provides explicit redaction, deletion, supersession, audit, and consolidation
workflows. These safeguards do not replace the privacy and data-use terms of the
AI host, operating system, source-control provider, or any other tool the user
chooses to invoke.

Support and privacy questions may be filed at
<https://github.com/TerminallyLazy/tree-ring-memory-codex-plugin/issues>.
Do not include secrets or private memory content in a public issue.
