# Tree Ring Memory Plugin Privacy Notice

Effective August 23, 2026

The Tree Ring Memory repository plugin packages instructions and local
lifecycle-hook registrations for AI coding agents. It does not operate a hosted
service, create a user account, collect analytics, send telemetry, or include a
remote MCP server.

The repository hooks run only when a session or subagent starts or stops. They
forward the host's lifecycle JSON through standard input to the separately
installed Tree Ring Memory CLI and wait synchronously for at most 10 seconds.
They do not register for user prompts, tool calls, or `SessionEnd`; persist hook
input; capture prompts or transcripts; or run in the background. The lifecycle
parser never inspects or persists `transcript_path`, `last_assistant_message`,
prompts, or transcript content.

Each stop event enforces one agent-mediated memory checkpoint. It asks the
active agent to evaluate already-grounded work rather than deriving a summary
from hook input. Only a concise, durable candidate classified as normal
sensitivity may be written automatically with strict `tree-ring capture`.
That command fixes agent scope, requires identity and provenance, tags the
memory as automatic capture, and rejects sensitive content. If no candidate
passes, no durable memory is created.

When an agent runs an explicit command in the separately installed Tree Ring
Memory CLI, including an identity-bound strict capture approved by the
checkpoint gates, the CLI stores accepted memory content in a local SQLite
database under the configured Tree Ring root. The project does not receive that
database or its contents. Data leaves the local environment only when the user
or another tool explicitly exports, syncs, publishes, or otherwise transmits it.

The plugin instructs agents to avoid transcripts, credentials, secrets, private
keys, raw chain-of-thought, and unnecessary sensitive personal data. It also
provides explicit redaction, deletion, supersession, audit, and consolidation
workflows. These safeguards do not replace the privacy and data-use terms of the
AI host, operating system, source-control provider, or any other tool the user
chooses to invoke.

Support and privacy questions may be filed at
<https://github.com/TerminallyLazy/Tree-Ring-Memory/issues>.
Do not include secrets, vulnerability details, or private memory content in a
public issue.
