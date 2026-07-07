# AI Native Landscape Issue

Status: submitted
Submitted: 2026-07-07T23:12:55Z
Outlet: AI Native Landscape
Public issue: https://github.com/rootsongjc/ai-native-landscape/issues/6
Evidence comment: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4909862599

## Submission Path

AI Native Landscape documents GitHub Issues as the recommended path for new
contributors adding projects. The issue form accepts project name, GitHub
repository, homepage, license, category, subcategory, bilingual description,
tags, and checklist confirmation.

The first CLI attempt passed the issue-form label `project-submission`, but
GitHub rejected it because that label is not available as a normal issue label.
The retry without a forced label created the issue successfully.

## Fit

- Category: `rag-knowledge`
- Subcategory: `agent-memory-context`
- License: MIT
- Repository: https://github.com/TerminallyLazy/Tree-Ring-Memory
- Homepage: https://terminallylazy.github.io/Tree-Ring-Memory/

Tree Ring Memory is a local-first memory lifecycle framework for AI agents. It
belongs in the knowledge and context area because it focuses on recall,
forgetting, audit, consolidation, and durable agent memory rather than acting as
a standalone agent runtime, vector database, or hosted SaaS.

## Verification

- Existing AI Native Landscape issue search for exact Tree Ring Memory returned
  no results.
- The expected project URL
  `https://landscape.jimmysong.io/projects/tree-ring-memory/` returned HTTP
  `404`, indicating the obvious slug is not already live.
- Tree Ring repository metadata verified public repository, MIT license,
  current activity on 2026-07-07, and homepage metadata.
- The public launch page returned HTTP `200`.
- The public `llms.txt` summary returned HTTP `200`.

## Submitted Body

### Project name

Tree Ring Memory

### GitHub repository

https://github.com/TerminallyLazy/Tree-Ring-Memory

### Homepage / Website

https://terminallylazy.github.io/Tree-Ring-Memory/

### Open source license

MIT

### Category

rag-knowledge

### Subcategory

agent-memory-context

### Why should this project be included?

EN: Tree Ring Memory is a framework-agnostic, local-first memory lifecycle
framework for AI agents. It provides a Rust CLI, SQLite/FTS recall, forgetting,
audit, consolidation, and portable skill/adapters so agent memory can persist
across sessions without becoming transcript dumps.

ZH: Tree Ring Memory 是一个面向 AI 代理的框架无关、本地优先记忆生命周期框架，提供 Rust CLI、SQLite/FTS
召回、遗忘、审计、整合以及可移植技能和适配器，让代理记忆能够跨会话保留，同时避免变成简单的对话转储。

### Tags

agent-memory, local-first, rust, cli, sqlite, fts, context-engineering,
ai-agents

### Submission checklist

- [x] The project has a publicly accessible GitHub repository
- [x] The project is actively maintained (commits in the last 6 months)
- [x] The project is open source (not a wrapper around a proprietary API)
- [x] I have searched and this project is not already listed

### Suggested classification

- Category: `rag-knowledge`
- Subcategory: `agent-memory-context`

### Verifiable sources

- Repository: https://github.com/TerminallyLazy/Tree-Ring-Memory
- Launch page: https://terminallylazy.github.io/Tree-Ring-Memory/
- llms.txt summary: https://terminallylazy.github.io/Tree-Ring-Memory/llms.txt
- Launch feedback issue: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26

### Notes

Tree Ring Memory is agent memory infrastructure, not a standalone autonomous
agent runtime, vector database, or hosted SaaS. The closest fit appears to be
AI-native knowledge/context tooling for agent memory lifecycle work.

## Follow-Up

Monitor maintainer response on issue `#6`. If the maintainer asks for a PR,
create paired `data/projects/tree-ring-memory.en.md` and
`data/projects/tree-ring-memory.zh.md` files and run `npm run validate` plus
`npm run build` in a fork before opening the PR.
