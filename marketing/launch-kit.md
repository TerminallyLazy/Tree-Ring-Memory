# Tree Ring Memory Launch Kit

This kit is for the first public campaign around Tree Ring Memory. It is
designed for developer communities where credibility matters more than hype:
show a working repo, make the local install obvious, explain the memory model,
and disclose affiliation.

## Campaign Spine

### One-Line Positioning

Tree Ring Memory is a framework-agnostic memory lifecycle layer for AI agents.

### Longer Positioning

Tree Ring Memory helps agents remember useful decisions, warnings,
preferences, and lessons without turning memory into transcript dumps. Fresh
memory stays detailed, older memory compresses into rings, important scars
remain visible, and durable truths become heartwood.

### Audience

- Developers building custom AI agent workflows.
- Agent framework authors who need a portable memory layer.
- Local-first and privacy-conscious AI builders.
- Rust CLI users who want inspectable agent infrastructure.
- Researchers and operators evaluating how memory changes agent behavior.

### Proof Points

- Local-first by default; no required cloud service.
- Rust-native public runtime.
- SQLite/FTS storage with recall filtering.
- Explainable recall with ring, scope, confidence, and ranking factors.
- First-class forgetting, redaction, supersession, audit, and maintenance.
- Deterministic consolidation without requiring an LLM.
- `tree-ring evidence` for source-linked evaluated outcomes.
- DOX and Revolve sync adapters.
- Framework discovery for common agent harnesses.
- Terminal onboarding and Ratatui operator console.
- Third-party discovery proof: Tree Ring Memory is indexed on Agent-Skills.md
  and merged into HeyClaude / Awesome Claude as a source-backed skills entry.

### Install Command

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

### Demo Commands

```bash
tree-ring init
tree-ring remember "Use project-scoped recall before changing release behavior." \
  --event-type lesson \
  --scope project \
  --project example-service \
  --tag release
tree-ring recall "release behavior" --project example-service
tree-ring evidence "A test run proved the new workflow fixed stale recall." \
  --outcome promoted \
  --evidence-ref evals/recall/run-042
tree-ring audit --audit-type sensitive
tree-ring tui
```

## Platform Rules Checked

- Hacker News `Show HN` is for things people can try and give feedback on:
  `https://news.ycombinator.com/showhn.html`
- Hacker News account page: `https://news.ycombinator.com/login`
- Reddit requires authentic participation and no spam:
  `https://redditinc.com/policies/reddit-rules`
- Reddit signup help: `https://support.reddithelp.com/hc/en-us/articles/360060420092-How-do-I-sign-up-for-a-Reddit-account`
- X signup: `https://help.x.com/en/using-x/create-x-account`
- X usernames are capped at 15 characters:
  `https://help.x.com/en/managing-your-account/x-username-rules`
- X profile/header dimensions:
  `https://help.x.com/en/managing-your-account/how-to-customize-your-profile`
- YouTube channel creation:
  `https://support.google.com/youtube/answer/1646861`
- YouTube thumbnails:
  `https://support.google.com/youtube/answer/72431`
- YouTube channel banner guidance:
  `https://support.google.com/youtube/answer/12950272`

## GitHub Launch Surface

### Repository Description

Framework-agnostic, local-first memory lifecycle for AI agents. Rust CLI,
SQLite/FTS recall, forgetting, audit, consolidation, DOX/Revolve adapters, and
terminal TUI.

### Topics

`ai-agents`, `agent-memory`, `local-first`, `rust`, `sqlite`, `cli`,
`ratatui`, `memory-management`, `dox`, `revolve`, `developer-tools`

### Pinned Issue

Title:

```text
Launch feedback: try Tree Ring Memory and tell us where agent memory breaks
```

Body:

````markdown
Tree Ring Memory is in protocol-preview status. The goal is simple: make agent
memory useful without turning it into a transcript dump.

Try the installer:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." --event-type lesson --scope project
tree-ring recall "release changes"
```

Feedback I am especially interested in:

- Which agent frameworks should get first-class bridge support?
- Where does the ring model feel too simple or too heavy?
- What should explainable recall show by default?
- What privacy and forgetting controls are missing?
- What would make this easy to adopt in your agent workflow?
````

## Hacker News

### Submission Title

```text
Show HN: Tree Ring Memory - memory lifecycle for AI agents
```

### URL

```text
https://terminallylazy.github.io/Tree-Ring-Memory/
```

### First Comment

````markdown
Hi HN, I built Tree Ring Memory because most agent memory systems I have used
either forget too aggressively or turn into transcript dumps.

The model is deliberately simple:

- fresh work stays detailed
- older learning compresses into rings
- important negative lessons become scars
- durable truths become heartwood
- speculative follow-ups stay as seeds

The current public runtime is Rust-native: CLI, SQLite/FTS storage, recall,
import/export, audit, deterministic consolidation, maintenance, DOX/Revolve
sync adapters, framework discovery, and a Ratatui terminal console.

It is protocol-preview software, not a hosted service. I am looking for
feedback from people building agent workflows: what should a portable,
local-first memory layer get right before deeper framework adapters land?

Install:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." --event-type lesson --scope project
tree-ring recall "release changes"
```

Repo: https://github.com/TerminallyLazy/Tree-Ring-Memory
Agent skill listing: https://agent-skills.md/skills/TerminallyLazy/tree-ring-memory-skill/tree-ring-memory
HeyClaude source listing: https://github.com/JSONbored/awesome-claude/blob/main/content/skills/tree-ring-memory.mdx
````

### HN Reply Angles

- If asked "why not vector DB?": Tree Ring Memory is the lifecycle and protocol
  layer: capture, scope, age, consolidate, audit, recall, and forget. Vector
  search can be an adapter, but it is not the memory policy.
- If asked "why Rust?": the public runtime is a local CLI/storage layer that
  benefits from a small binary, deterministic behavior, and easy inspection.
- If asked "what is protocol-preview?": core concepts and CLI are usable, but
  first-class bridges for more agent harnesses are still being shaped.

## Reddit

Do not blast the same post into many subreddits. Tailor the angle, disclose
that it is your project, and prefer weekly showcase/self-promotion threads when
communities require them.

### r/rust

Title:

```text
I built a Rust-native CLI for local AI agent memory lifecycle
```

Body:

````markdown
I have been working on Tree Ring Memory, a Rust-native local memory lifecycle
layer for AI agents.

The Rust workspace owns the public runtime now: CLI, SQLite/FTS storage, recall
filtering, JSONL import/export, audit, deterministic consolidation, maintenance,
DOX/Revolve source adapters, framework discovery, and a Ratatui operator
console.

The idea is not "store every chat." It is memory aging:

- fresh task context stays detailed
- older learning compresses into rings
- failures/regressions become scars
- durable facts become heartwood
- future hypotheses stay as seeds

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Website:
https://terminallylazy.github.io/Tree-Ring-Memory/

Install:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring tui
```

I would value feedback on the Rust CLI/storage shape, the SQLite/FTS approach,
and what should stay deterministic versus adapter-driven.

The portable skill package is also indexed here if you want to inspect the
agent-facing instructions before installing:
https://agent-skills.md/skills/TerminallyLazy/tree-ring-memory-skill/tree-ring-memory
````

### r/LocalLLaMA

Title:

```text
Local-first memory lifecycle for AI agents, without transcript dumping
```

Body:

```markdown
I built Tree Ring Memory for local/private agent workflows where "memory" should
not mean uploading transcripts or keeping every chat forever.

It is a framework-agnostic Rust CLI with local SQLite/FTS storage. Agents can
remember durable lessons, recall project-specific context, record evidence from
evaluations, audit sensitive/stale memory, consolidate older records, and forget
or redact memory explicitly.

The model is tree-ring based:

- cambium: fresh active context
- outer/inner rings: compressed older learning
- scars: negative lessons worth keeping visible
- heartwood: durable truths
- seeds: unresolved future ideas

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Website:
https://terminallylazy.github.io/Tree-Ring-Memory/

I am looking for feedback from people running local agent stacks: what adapter
or workflow would make this useful in your setup?

For agent-skill users, the source-backed HeyClaude entry is here:
https://github.com/JSONbored/awesome-claude/blob/main/content/skills/tree-ring-memory.mdx
```

### r/opensource

Title:

```text
Tree Ring Memory: open-source local memory lifecycle for AI agents
```

Body:

```markdown
Tree Ring Memory is an open-source, framework-agnostic memory lifecycle layer
for AI agents.

It is local-first and Rust-native. The current CLI handles storage, recall,
audit, consolidation, maintenance, JSONL import/export, source-linked evidence,
DOX/Revolve sync, and a terminal TUI.

The problem it is trying to solve: agent memory often becomes either uselessly
forgetful or a raw transcript dump. Tree Ring Memory treats memory as something
that ages, compresses, keeps scars, preserves durable truths, and can be
forgotten.

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Website:
https://terminallylazy.github.io/Tree-Ring-Memory/

Feedback wanted: docs clarity, install friction, privacy/forgetting model, and
which agent integrations should come first.
```

### r/commandline

Title:

```text
Tree Ring Memory: a Rust CLI/TUI for local AI-agent memory
```

Body:

````markdown
I built Tree Ring Memory as a command-line memory lifecycle tool for AI-agent
workflows.

The public runtime is Rust-native and local-first:

- explicit durable writes through `tree-ring remember`
- scoped SQLite/FTS recall through `tree-ring recall`
- audit, consolidation, maintenance, forgetting, and redaction flows
- JSONL import/export for portability
- DOX/Revolve source adapters
- Ratatui `tree-ring tui` for terminal review

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Install:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring tui
```

Feedback wanted: command names, output shape, install friction, TUI usefulness,
and whether the import/export flow feels Unix-friendly.
````

### r/AI_Agents

Title:

```text
Framework-agnostic memory lifecycle for AI agents
```

Body:

````markdown
I built Tree Ring Memory as a local-first memory lifecycle layer for AI-agent
workflows.

It is not another agent framework. It handles the memory substrate beside one:
remember, recall, evidence, audit, consolidate, forget, redact, import, export,
and adapt.

The current runtime is a Rust CLI with local SQLite/FTS storage. The model is
tree-ring based: fresh context stays detailed, older learning compresses,
negative lessons stay visible as scars, durable truths become heartwood, and
unresolved follow-ups remain seeds.

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Agent-facing package:
https://agent-skills.md/skills/TerminallyLazy/tree-ring-memory-skill/tree-ring-memory

I am looking for feedback from people building agents: where should adapter
writes sit so memory is useful without becoming automatic transcript hoarding?
````

## X

### Launch Thread

1.

```text
Tree Ring Memory is a memory lifecycle layer for AI agents.

Not a vector DB.
Not transcript storage.

Fresh work stays detailed. Older learning compresses. Failures become scars. Durable truths become heartwood.

https://terminallylazy.github.io/Tree-Ring-Memory/
```

2.

```text
Agent memory has two common failure modes:

1. it forgets everything useful between runs
2. it remembers everything as raw transcript sludge

Tree Ring Memory is a protocol and CLI for the middle path: scoped, explainable, auditable, forgettable memory.
```

3.

```text
The rings:

- cambium: fresh active context
- outer/inner: older compressed learning
- scar: failure or regression warnings
- heartwood: durable truths
- seed: unresolved future ideas

The point is lifecycle, not hoarding.
```

4.

```text
The public runtime is Rust-native:

- CLI
- SQLite/FTS storage
- explainable recall
- JSONL import/export
- audit + maintenance
- deterministic consolidation
- DOX/Revolve adapters
- framework discovery
- Ratatui terminal console
```

5.

```text
Install:

curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh

Then:
tree-ring init
tree-ring remember "Use project-scoped recall before risky changes." --event-type lesson --scope project
tree-ring recall "risky changes"
```

6.

```text
The privacy boundary matters:

Tree Ring does not scrape transcripts, run a hidden recorder, or turn terminal pulses into durable memory.

Writes are explicit: remember, evidence, import, consolidation, or deliberate agent action.
```

7.

```text
The part I want feedback on:

What should a portable memory layer get right before deeper adapters land?

Codex? Claude Code? Agent Zero? OpenCode? LangGraph? MCP?

Try it, break it, tell me what feels wrong.
https://terminallylazy.github.io/Tree-Ring-Memory/
```

### Standalone X Post

```text
Tree Ring Memory is a Rust-native memory lifecycle layer for AI agents.

Useful decisions, warnings, preferences, and evidence without transcript dumping.

CLI, SQLite/FTS recall, audit, consolidation, forgetting, TUI.

https://terminallylazy.github.io/Tree-Ring-Memory/
```

## Bluesky / Mastodon

```text
I opened Tree Ring Memory: a local-first, Rust-native memory lifecycle layer for AI agents.

It helps agents preserve useful decisions, warnings, preferences, and evidence without turning memory into transcript dumps.

Rust CLI, SQLite/FTS, explainable recall, audit, consolidation, forgetting, DOX/Revolve adapters, and a terminal TUI.

https://github.com/TerminallyLazy/Tree-Ring-Memory
```

## YouTube

### First Video Title

```text
Tree Ring Memory: Local-first memory lifecycle for AI agents
```

### Thumbnail

Use `marketing/assets/youtube-thumbnail-1920x1080.png`.

### Description

```markdown
Tree Ring Memory is a framework-agnostic, local-first memory lifecycle layer for AI agents.

It helps agents remember useful decisions, warnings, preferences, and evaluated lessons without turning memory into transcript dumps.

In this demo:
- install the Rust-native CLI
- initialize local memory
- remember a project lesson
- recall scoped memory
- record evidence
- run audit
- open the Ratatui terminal console

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Website:
https://terminallylazy.github.io/Tree-Ring-Memory/

Install:
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh

Commands:
tree-ring init
tree-ring remember "Use project-scoped recall before risky changes." --event-type lesson --scope project
tree-ring recall "risky changes"
tree-ring evidence "The eval passed after the fix." --outcome promoted --evidence-ref evals/run-042
tree-ring audit --audit-type sensitive
tree-ring tui

Chapters:
00:00 Why agent memory needs a lifecycle
00:40 Install
01:15 Initialize local memory
01:50 Remember and recall
02:40 Evidence and scars
03:20 Audit and forgetting
04:00 Terminal console
04:40 What feedback I need
```

### 5-Minute Demo Script

```text
Most AI agent memory systems fail in one of two ways. They either forget the
things that matter between runs, or they remember everything as a transcript
dump.

Tree Ring Memory is a different model: memory should age.

Fresh work stays detailed. Older learning compresses into rings. Important
failures remain visible as scars. Durable truths become heartwood. Future
ideas stay as seeds until they are reviewed.

This is the current Rust-native CLI.

First I install it, then initialize a local memory root. Tree Ring uses local
SQLite and FTS by default, so there is no required hosted service.

Now I can store a small project lesson. This is intentionally concise: the
lesson, not the whole conversation.

Next I recall by query. Recall results carry their ring, scope, confidence, and
ranking signals so memory is inspectable instead of magical.

For evaluated outcomes, I use `tree-ring evidence`. A promoted outcome becomes
durable heartwood. A rejected outcome becomes a scar, so future agents can see
the warning instead of repeating the same failed path.

Tree Ring also has audit, maintenance, consolidation, import/export, DOX and
Revolve adapters, framework discovery, and a terminal TUI.

The privacy boundary is important: Tree Ring does not scrape transcripts or run
a hidden recorder. Durable writes are explicit.

The project is in protocol-preview status. I am looking for feedback from
people building agent workflows: what should a portable, local-first memory
layer get right before deeper framework adapters land?
```

## Product Hunt

### Product Name

```text
Tree Ring Memory
```

### Tagline

```text
Framework-agnostic memory lifecycle for AI agents
```

### Description

```text
Tree Ring Memory is a local-first, Rust-native memory lifecycle layer for AI
agents. It helps agents remember useful decisions, warnings, preferences, and
evidence without becoming transcript dumps. Includes CLI, SQLite/FTS recall,
audit, deterministic consolidation, forgetting, DOX/Revolve adapters, and a
terminal TUI.
```

### Maker Comment

```markdown
I built Tree Ring Memory because agent memory needs lifecycle rules, not just
storage.

The model is inspired by tree rings:

- fresh work stays detailed
- older learning compresses into rings
- failures become scars
- durable truths become heartwood
- future ideas stay as seeds

The public runtime is Rust-native and local-first. I am especially interested
in feedback from developers building agent workflows: which adapters should
come first, and what should explainable recall show by default?
```

## LinkedIn

```text
I opened Tree Ring Memory, a framework-agnostic memory lifecycle layer for AI agents.

The premise is simple: agent memory should not be a raw transcript dump.

Fresh work should stay detailed. Older learning should compress. Important failures should remain visible. Durable truths should be preserved. Sensitive or wrong memory should be forgettable.

The current release is a local-first, Rust-native CLI with SQLite/FTS storage, explainable recall, audit, maintenance, deterministic consolidation, JSONL import/export, DOX/Revolve adapters, framework discovery, and a terminal TUI.

The project is in protocol-preview status, and I am looking for feedback from people building or operating agent workflows.

https://terminallylazy.github.io/Tree-Ring-Memory/
```

## Dev.to / Hashnode / Medium

### Article Title

```text
Why AI Agent Memory Should Age Like Tree Rings
```

### Outline

1. The agent memory trap: forgetting everything or storing everything.
2. Why raw transcripts are not durable memory.
3. The ring model: cambium, outer, inner, heartwood, scars, seeds.
4. Explainable recall beats magical recall.
5. Forgetting, redaction, and supersession are product features.
6. Why Tree Ring is local-first and Rust-native.
7. Demo: install, remember, recall, evidence, audit, TUI.
8. What feedback is needed before deeper adapters.

### Opening

```markdown
Most AI agent memory systems have a lifecycle problem.

They either forget the useful parts of prior work, or they preserve too much
raw context and call it memory. Neither shape is good enough for agents that
touch real code, private projects, release workflows, or user preferences.

Tree Ring Memory starts from a different premise: memory should age.
```

## Newsletter / Directory Pitch

```text
Tree Ring Memory is a new open-source, local-first memory lifecycle layer for
AI agents. It is framework-agnostic and Rust-native, with a CLI, SQLite/FTS
recall, audit, forgetting, deterministic consolidation, source-linked evidence,
DOX/Revolve adapters, and a Ratatui terminal console.

The project is in protocol-preview status and looking for feedback from agent
framework authors, local AI builders, and developer-tool users.

Website: https://terminallylazy.github.io/Tree-Ring-Memory/
Repo: https://github.com/TerminallyLazy/Tree-Ring-Memory
```

## Outreach Targets

### Priority

- Hacker News Show HN
- X launch thread
- Reddit community-specific posts
- YouTube demo
- GitHub repo description/topics/social preview

### Developer Blogs

- Dev.to
- Hashnode
- Medium
- Substack

### Social / Community

- Bluesky
- Mastodon / Hachyderm
- LinkedIn
- Discord server once support volume exists

### Launch / Directories

- Product Hunt
- AlternativeTo
- Awesome AI agents lists
- Awesome Rust lists where relevant
- AI tooling newsletters
- Rust newsletters
- Local-first software communities

## Posting Order

1. Update GitHub description, topics, social preview, and first feedback issue.
2. Create social accounts and complete profile assets.
3. Record/upload YouTube demo.
4. Post HN `Show HN`.
5. Post X launch thread and pin it.
6. Post one tailored Reddit thread at a time.
7. Publish the long-form article.
8. Pitch newsletters and directories with the short directory pitch.
9. Launch Product Hunt only after there is a video, a stable release artifact,
   and enough docs for first-time users.

## Measurement

Track this manually for the first launch:

- GitHub stars, forks, issues, and discussions.
- Installer failures or confusion.
- HN comments and recurring objections.
- Reddit removals, moderator feedback, and high-quality replies.
- X bookmarks/reposts/replies from agent/Rust/local-first builders.
- YouTube watch retention and comments.
- Adapter requests by framework.

The most important launch metric is not raw impressions. It is evidence of
which memory workflows developers actually want Tree Ring to support next.
