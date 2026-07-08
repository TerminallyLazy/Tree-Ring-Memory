# Tree Ring Memory Channel Playbook

This playbook turns the launch kit into execution-ready posts. It is written
for manual posting from verified owner-controlled accounts.

## Current Public Surfaces

- Website: <https://terminallylazy.github.io/Tree-Ring-Memory/>
- Repository: <https://github.com/TerminallyLazy/Tree-Ring-Memory>
- Release: <https://github.com/TerminallyLazy/Tree-Ring-Memory/releases/tag/v0.11.0>
- Discussion: <https://github.com/TerminallyLazy/Tree-Ring-Memory/discussions/27>
- Feedback issue: <https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26>
- Press kit: <https://terminallylazy.github.io/Tree-Ring-Memory/press-kit.md>
- LLM summary: <https://terminallylazy.github.io/Tree-Ring-Memory/llms.txt>
- Feed: <https://terminallylazy.github.io/Tree-Ring-Memory/feed.xml>
- Agent-Skills.md listing:
  <https://agent-skills.md/skills/TerminallyLazy/tree-ring-memory-skill/tree-ring-memory>
- HeyClaude / Awesome Claude source entry:
  <https://github.com/JSONbored/awesome-claude/blob/main/content/skills/tree-ring-memory.mdx>

## Official Guardrails Checked

- Hacker News Show HN: post a tryable project whose title begins with
  `Show HN`, and do not ask for upvotes.
  <https://news.ycombinator.com/showhn.html>
- Reddit: stay authentic, avoid spam, read each community rule before posting,
  and do not mislead users about affiliation.
  <https://redditinc.com/policies/reddit-rules>
- Reddit organic guidance: many communities do not allow self-promotion, so
  use relevant comments and approved showcase threads.
  <https://redditinc.com/hubfs/Reddit%20Inc/Content/Reddit%20Pros%20organic%20playbook.pdf>
- X account/profile setup: X supports 400x400 profile images, 1500x500 headers,
  and 160-character bios.
  <https://help.x.com/en/managing-your-account/how-to-customize-your-profile>
- YouTube upload details: upload through YouTube Studio, set title,
  description, thumbnail, and tags.
  <https://support.google.com/youtube/answer/57407>
- YouTube verification: custom thumbnails and videos over 15 minutes require
  channel verification.
  <https://support.google.com/youtube/answer/171664>
- Product Hunt: prepare URL, product details, media, maker comment, and launch
  materials before submitting a new product.
  <https://www.producthunt.com/launch>
- DEV: posts are Markdown-based and can be drafted, scheduled, and published
  from the editor.
  <https://dev.to/help/writing-editing-scheduling>

## Wave Plan

| Wave | Outlet | Goal | Primary URL | Asset | Copy |
| --- | --- | --- | --- | --- | --- |
| 0 | GitHub Discussion | Public launch conversation | Discussion #27 | none | Live |
| 0 | Atom feed | Syndication and crawler surface | `docs/feed.xml` | none | Live |
| 1 | Hacker News | Technical critique and early users | Website | Open Graph card | Show HN copy |
| 1 | X | Developer-tool awareness | Website | Open Graph card, header | Launch thread |
| 1 | YouTube | Proof that it runs | Demo MP4 | Thumbnail | Video package |
| 2 | Reddit | Rust/local-agent feedback | Repo or website | Reddit card | Tailored posts |
| 2 | Bluesky/Mastodon | Open-source developer reach | Repo | Square card | Short launch note |
| 3 | Dev.to/Hashnode/Medium | Durable essay traffic | Website | Square banner | Explainer article |
| 3 | Newsletters/directories | Curated discovery | Press kit | Open Graph card | Pitch snippets |
| 4 | Product Hunt | Broader launch moment | Website | Product gallery | Maker comment |

## Hacker News

Post URL:

```text
https://terminallylazy.github.io/Tree-Ring-Memory/
```

Title:

```text
Show HN: Tree Ring Memory - memory lifecycle for AI agents
```

First comment:

````markdown
Hi HN, I built Tree Ring Memory because most agent memory systems I have used
either forget too aggressively or turn into transcript dumps.

The model is deliberately simple:

- fresh work stays detailed
- older learning compresses into rings
- important negative lessons become scars
- durable truths become heartwood
- speculative follow-ups stay as seeds

The public runtime is Rust-native: CLI, SQLite/FTS storage, recall, audit,
forgetting, deterministic consolidation, maintenance, DOX/Revolve sync
adapters, framework discovery, and a Ratatui terminal console.

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
Release: https://github.com/TerminallyLazy/Tree-Ring-Memory/releases/tag/v0.11.0
Agent skill listing: https://agent-skills.md/skills/TerminallyLazy/tree-ring-memory-skill/tree-ring-memory
HeyClaude source entry: https://github.com/JSONbored/awesome-claude/blob/main/content/skills/tree-ring-memory.mdx
````

Execution notes:

- Use an established maker account if possible.
- Stay in the thread for the first two hours.
- Do not ask friends or followers to upvote.
- Reply with implementation details and tradeoffs, not slogans.

## Reddit

Do not cross-post the same copy everywhere. Post only where rules allow
projects, launches, or self-promotion.

Current public rule checks are summarized in
`marketing/reddit-rule-check-2026-07-08.md`. Reddit copy in this playbook is a
brief, not paste-ready text. Before posting, the owner should read current
rules from an authenticated session and rewrite the post in their own voice.

### r/rust

Title:

```text
I built a Rust-native CLI for local AI agent memory lifecycle
```

Body:

````markdown
I built Tree Ring Memory, a Rust-native local memory lifecycle layer for AI
agents.

The Rust workspace owns the public runtime: CLI, SQLite/FTS storage, recall
filtering, JSONL import/export, audit, deterministic consolidation,
maintenance, DOX/Revolve source adapters, framework discovery, and a Ratatui
operator console.

The idea is not "store every chat." It is memory aging:

- fresh task context stays detailed
- older learning compresses into rings
- failures/regressions become scars
- durable facts become heartwood
- future hypotheses stay as seeds

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Release:
https://github.com/TerminallyLazy/Tree-Ring-Memory/releases/tag/v0.11.0

Install:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring tui
```

I would value feedback on the Rust CLI/storage shape, the SQLite/FTS approach,
and what should stay deterministic versus adapter-driven.
````

### r/LocalLLaMA

Title:

```text
Local-first memory lifecycle for AI agents, without transcript dumping
```

Body:

```markdown
I built Tree Ring Memory for local/private agent workflows where "memory"
should not mean uploading transcripts or keeping every chat forever.

It is a framework-agnostic Rust CLI with local SQLite/FTS storage. Agents can
remember durable lessons, recall project-specific context, record evidence from
evaluations, audit sensitive/stale memory, consolidate older records, and
forget or redact memory explicitly.

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
```

### r/opensource

Title:

```text
Tree Ring Memory: open-source local memory lifecycle for AI agents
```

Body:

```markdown
I opened Tree Ring Memory, a framework-agnostic memory lifecycle layer for AI
agents.

It is local-first and Rust-native. The current CLI handles storage, recall,
audit, consolidation, maintenance, JSONL import/export, source-linked evidence,
DOX/Revolve sync, framework discovery, and a terminal TUI.

The problem it is trying to solve: agent memory often becomes either uselessly
forgetful or a raw transcript dump. Tree Ring Memory treats memory as something
that ages, compresses, keeps scars, preserves durable truths, and can be
forgotten.

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

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

- `tree-ring remember` for explicit durable lessons, warnings, preferences,
  evidence, scars, and seeds
- `tree-ring recall` for scoped SQLite/FTS recall
- `tree-ring audit` and maintenance flows for stale/sensitive memory
- JSONL import/export for portability
- DOX/Revolve source adapters
- Ratatui `tree-ring tui` for reviewing memory state in the terminal

It is not a hidden transcript recorder. The CLI is meant to make memory
inspectable, explainable, and forgettable.

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Install:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring remember "Prefer project-scoped recall before risky changes." --event-type lesson --scope project
tree-ring recall "risky changes"
tree-ring tui
```

I would value command-line feedback: naming, output shape, install friction,
TUI usefulness, and what a Unix-friendly memory export/import flow should look
like.
````

Execution notes:

- Hold for this launch. Public rules checked on 2026-07-08 disallow most
  generative-AI-related projects and projects newer than 30 days.
- Revisit only after the project has stronger adoption or moderator approval.

### r/AI_Agents

Title:

```text
Framework-agnostic memory lifecycle for AI agents
```

Body:

````markdown
I built Tree Ring Memory as a local-first memory lifecycle layer for AI-agent
workflows.

It is not another agent framework. It is the memory substrate beside one:
explicit writes, scoped recall, evidence records, audit, consolidation,
forgetting, redaction, JSONL import/export, and adapter hooks.

The current runtime is a Rust CLI with local SQLite/FTS storage. The model uses
tree-ring lifecycle states:

- fresh context stays detailed
- older learning compresses into rings
- negative lessons stay visible as scars
- durable truths become heartwood
- unresolved follow-ups remain seeds

I am looking for feedback from people building agents: where should the adapter
boundary sit so memory is useful without becoming automatic transcript hoarding?
````

First comment:

````markdown
Maintainer note: Tree Ring Memory is my project.

Repo:
https://github.com/TerminallyLazy/Tree-Ring-Memory

Portable skill package:
https://agent-skills.md/skills/TerminallyLazy/tree-ring-memory-skill/tree-ring-memory

Source-backed HeyClaude entry:
https://github.com/JSONbored/awesome-claude/blob/main/content/skills/tree-ring-memory.mdx
````

Execution notes:

- Read current `r/AI_Agents` self-promotion and project-post rules before
  posting.
- Put project links in a comment or weekly project display thread, not directly
  in the post body.
- Disclose maintainer affiliation in the post or first comment.
- Stay focused on agent memory lifecycle and adapters, not broad AI hype.

## X

Pinned profile link:

```text
https://terminallylazy.github.io/Tree-Ring-Memory/
```

Launch thread:

```text
Tree Ring Memory is a memory lifecycle layer for AI agents.

Not a vector DB.
Not transcript storage.

Fresh work stays detailed. Older learning compresses. Failures become scars. Durable truths become heartwood.

https://terminallylazy.github.io/Tree-Ring-Memory/
```

```text
Agent memory has two common failure modes:

1. it forgets everything useful between runs
2. it remembers everything as raw transcript sludge

Tree Ring Memory is a protocol and CLI for the middle path: scoped, explainable, auditable, forgettable memory.
```

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

```text
The privacy boundary matters:

Tree Ring does not scrape transcripts, run a hidden recorder, or turn terminal pulses into durable memory.

Writes are explicit: remember, evidence, import, consolidation, or deliberate agent action.
```

```text
The part I want feedback on:

What should a portable memory layer get right before deeper adapters land?

Codex? Claude Code? Agent Zero? OpenCode? LangGraph? MCP?

Try it, break it, tell me what feels wrong.
https://github.com/TerminallyLazy/Tree-Ring-Memory/discussions/27
```

## YouTube

Use:

- Video: `outputs/marketing/youtube-demo/tree-ring-memory-demo.mp4`
- Thumbnail: `marketing/assets/youtube-thumbnail-1920x1080.png`
- Title: `marketing/youtube/title.txt`
- Description: `marketing/youtube/description.md`
- Tags: `marketing/youtube/tags.txt`
- Captions: `marketing/youtube/captions.srt`

Upload order:

1. Create or verify the YouTube channel.
2. Upload the MP4 in YouTube Studio.
3. Set the prepared title and description.
4. Upload thumbnail after channel verification if needed.
5. Add tags under "Show more".
6. Upload captions.
7. Set visibility to unlisted for a final watch-through.
8. Publish and add the public URL to `marketing/submission-ledger.csv`.

## Product Hunt

Do not launch until the YouTube demo URL exists. Product Hunt is better as a
coordinated moment, not a placeholder listing.

Product name:

```text
Tree Ring Memory
```

Tagline:

```text
Framework-agnostic memory lifecycle for AI agents
```

Maker comment:

```markdown
I built Tree Ring Memory because agent memory needs lifecycle rules, not just
storage.

The model is inspired by tree rings: fresh work stays detailed, older learning
compresses into rings, failures become scars, durable truths become heartwood,
and future ideas stay as seeds.

The public runtime is Rust-native and local-first. I am especially interested
in feedback from developers building agent workflows: which adapters should
come first, and what should explainable recall show by default?
```

## Developer Blogs

Canonical title:

```text
Why AI Agent Memory Should Age Like Tree Rings
```

Canonical URL:

```text
https://terminallylazy.github.io/Tree-Ring-Memory/launch/tree-ring-memory-framework.md
```

Opening:

```markdown
Most AI agent memory systems have a lifecycle problem.

They either forget the useful parts of prior work, or they preserve too much
raw context and call it memory. Neither shape is good enough for agents that
touch real code, private projects, release workflows, or user preferences.

Tree Ring Memory starts from a different premise: memory should age.
```

## Direct Outreach

Use the press kit when pitching newsletters, directories, maintainers, and
awesome-list curators. Keep the note short, disclose affiliation, and ask for
feedback or inclusion rather than asking for promotion.

Short note:

```text
I opened Tree Ring Memory, a local-first Rust-native memory lifecycle layer for
AI agents. It focuses on aging, recall, audit, forgetting, evidence, and
framework-agnostic integration rather than raw transcript storage.

Launch page: https://terminallylazy.github.io/Tree-Ring-Memory/
Repo: https://github.com/TerminallyLazy/Tree-Ring-Memory
Press kit: https://terminallylazy.github.io/Tree-Ring-Memory/press-kit.md
```
