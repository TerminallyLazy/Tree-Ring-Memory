# The Next AI Submission

Submission target: <https://www.thenextai.com/submit-ai-tool/>

Current status: free listing form accepted by the public Google Apps Script
endpoint on 2026-07-07 with HTTP `200` and response body `{"success":true}`.
The form states that free basic listings are manually reviewed and typically
published within 24-48 hours.

The required contact email field used the owner's public GitHub profile contact.
Do not store that address, submission tokens, or any private inbox details in
this repository.

## Submitted Fields

- Tool name: Tree Ring Memory
- Website URL: <https://terminallylazy.github.io/Tree-Ring-Memory/>
- Category: Developer Tools
- Pricing model: Open Source
- Short description: Local-first memory lifecycle framework for AI agents.
- Logo URL:
  <https://terminallylazy.github.io/Tree-Ring-Memory/assets/tree-ring-memory-icon.png>
- Tags: AI agents, developer tools, local-first, Rust, CLI, memory, SQLite,
  open source
- Plan: Free Basic

## Full Description

Tree Ring Memory is a framework-agnostic, local-first memory lifecycle layer for
AI agents. It helps agent workflows capture, recall, audit, consolidate, and
forget useful project memory without turning memory into transcript dumps. The
public runtime is Rust-native with a CLI, SQLite/FTS storage, explainable
recall, deterministic consolidation, redaction and deletion support,
DOX/Revolve adapters, Homebrew install support, and a terminal TUI for operator
workflows. It is aimed at developers building or operating AI agent systems that
need durable local context and auditable recall.

## Evidence

- Submission page confirmed free listings require no account or credit card.
- Form payload was posted as `text/plain` JSON to the page's public endpoint,
  matching the site JavaScript.
- Endpoint response: HTTP `200`, body `{"success":true}`.

## Follow-Up

- Watch for approval email or public page at
  `https://www.thenextai.com/ai-tools/tree-ring-memory/`.
- If approved, update the queue row from `Submitted` to `Live` and add the
  public listing URL.
