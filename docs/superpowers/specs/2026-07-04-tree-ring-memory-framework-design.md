# Tree Ring Memory Framework Design

## Status

Approved direction: protocol-first, framework-agnostic open-source memory framework with a Python reference implementation planned after this spec is reviewed.

This spec redesigns the Agent Zero Tree Ring Memory plugin into a portable framework that any AI agent workflow can adopt without depending on Agent Zero.

## Product Positioning

Tree Ring Memory is a portable memory lifecycle framework for AI agents.

It is not another vector database, chat transcript archive, or agent dashboard. It defines how agent memory should be captured, scoped, aged, consolidated, recalled, audited, and forgotten.

The core promise:

> Agents should remember like a tree preserves rings: fresh work stays rich, older learning compresses, important scars remain visible, and durable truths become heartwood.

## Primary Users

- Agent framework authors who need a memory layer they can embed or adapt.
- Developers building custom agent workflows with local or private memory needs.
- Teams that need inspectable, testable, privacy-aware agent memory.
- Researchers evaluating how memory changes agent behavior over time.

## Design Goals

- Framework-agnostic: useful with Agent Zero, LangChain, CrewAI, AutoGen, custom agents, MCP workflows, CLI agents, and future runtimes.
- Local-first: no external service required for v1.
- Explainable: every recall result should include evidence, ring, scope, confidence, and ranking factors.
- Forgettable: memory deletion, redaction, expiration, and supersession are first-class.
- Testable: the framework must prove memory quality through eval fixtures, not just store data.
- Privacy-aware: secrets and sensitive data fail closed by default.
- Interoperable: import/export and event envelopes should be stable enough for third-party adapters.

## Non-Goals For The First Open-Source Release

- Hosted cloud memory service.
- Browser extension.
- Mandatory vector database.
- Agent identity graph.
- Full UI workbench as the core product.
- Framework-specific runtime coupling.
- Automatic ingestion of every file or transcript.
- Silent sensitive data collection.

## HMW Framing

How might we help any AI agent preserve, compress, recall, and safely forget high-value learning across workflows without tying memory to one agent framework or turning memory into a transcript dump?

## Chosen Architecture

The first framework release should be a protocol-first core with a Python reference implementation.

The protocol defines the durable concepts:

- memory events
- source evidence
- scopes
- rings
- retention
- sensitivity
- supersession
- consolidation
- recall requests
- recall results
- audit findings
- import/export envelopes
- eval scenarios

The Python implementation proves the protocol is usable and provides the first production-ready local backend.

## Framework Layers

### 1. Memory Protocol

The protocol is the stable contract.

It defines:

- `MemoryEvent`
- `MemorySource`
- `MemoryLink`
- `MemoryReview`
- `RecallQuery`
- `RecallResult`
- `ConsolidationRecord`
- `AuditFinding`
- `MemoryExport`
- `MemoryEvalScenario`

The protocol should be documented in Markdown and represented as JSON Schema.

### 2. Core Reference Library

The first implementation should be Python.

Package name recommendation: `tree-ring-memory`

Python import recommendation:

```python
from tree_ring_memory import TreeRingMemory
```

Core responsibilities:

- validate protocol objects
- store and retrieve memories
- run deterministic sensitivity checks
- rank recall results
- consolidate rings
- forget, redact, expire, and supersede memories
- export and import memory bundles
- run memory eval scenarios

### 3. Local Storage Adapter

The first storage adapter should be SQLite + JSONL mirrors.

SQLite provides:

- durable local storage
- FTS search
- migration support
- low operational burden

JSONL mirrors provide:

- portable backups
- human-reviewable event trails
- git-friendly exports

Vector search should be an optional adapter, not a core requirement.

### 4. Integration Adapters

Adapters should be thin wrappers over the protocol.

Initial adapters:

- generic Python agent adapter
- CLI adapter
- Agent Zero adapter
- LangChain/LangGraph adapter
- MCP server adapter

Later adapters:

- AutoGen
- CrewAI
- OpenAI Agents SDK
- TypeScript SDK
- local sidecar daemon

### 5. Eval Harness

The eval harness is part of the core credibility story.

It should include scenario fixtures that test:

- relevant recall
- sensitive filtering
- stale memory suppression
- supersession behavior
- scars surfacing for failure-like queries
- heartwood surfacing for durable constraints
- consolidation idempotency
- import/export round trips

The framework should ship with a `tree-ring eval` command and fixture format.

## Memory Ring Model

### Cambium

Fresh active memory. Used for current or recent work.

Examples:

- current task decisions
- fresh tool results
- user corrections
- active assumptions

### Outer Rings

Recent summarized memory.

Examples:

- daily summaries
- recent project decisions
- active lessons

### Inner Rings

Older compressed memory.

Examples:

- monthly patterns
- older but still relevant architecture decisions
- repeated lessons

### Heartwood

Durable truths.

Examples:

- user-confirmed preferences
- stable project constraints
- validated procedures
- promoted experiment lessons

Heartwood should require user confirmation, strong evidence, or repeated validation.

### Scars

Important negative memory.

Examples:

- failed approaches
- regressions
- privacy mistakes
- broken workflows
- rejected experiments with reusable warning value

Scars should never be auto-deleted unless the user explicitly requests it.

### Seeds

Unresolved future possibilities.

Examples:

- hypotheses
- roadmap ideas
- deferred alternatives
- candidate experiments

Seeds should be periodically reviewed, promoted, archived, or forgotten.

## Protocol Object: MemoryEvent

Required logical fields:

```json
{
  "id": "mem_2026_07_04_000001",
  "created_at": "2026-07-04T18:30:00-04:00",
  "updated_at": "2026-07-04T18:30:00-04:00",
  "project": "optional project id",
  "agent_profile": "optional agent id or profile",
  "scope": "global|project|agent|session|workflow|tool|eval|manual",
  "ring": "cambium|outer|inner|heartwood|scar|seed",
  "event_type": "decision|lesson|warning|preference|tool_result|eval_result|summary|hypothesis|correction",
  "summary": "Concise memory statement.",
  "details": "Optional longer details.",
  "source": {
    "type": "user|tool|file|eval|manual|adapter|api",
    "ref": "path, run id, checkpoint id, URL, or adapter-specific pointer",
    "quote": "short evidence excerpt when allowed"
  },
  "tags": ["memory", "project"],
  "salience": 0.8,
  "confidence": 0.7,
  "sensitivity": "normal|private|secret|health|financial|legal|personal_identifier",
  "retention": "ephemeral|normal|durable|user_pinned|forget_after_date",
  "expires_at": null,
  "supersedes": [],
  "superseded_by": null,
  "links": [],
  "review": {
    "needs_review": false,
    "review_reason": null,
    "reviewed_at": null,
    "reviewed_by": null
  }
}
```

## Core API

The reference library should expose a small stable surface:

```python
memory = TreeRingMemory.open("./.tree-ring")

memory.remember(...)
memory.recall(...)
memory.consolidate(...)
memory.forget(...)
memory.audit(...)
memory.import_bundle(...)
memory.export_bundle(...)
memory.run_eval(...)
```

### remember

Stores meaningful memory, not raw transcript by default.

Required behavior:

- validate schema
- classify sensitivity
- block obvious secrets by default
- assign ring when absent
- write storage row
- update search index
- return memory id and warnings

### recall

Retrieves relevant memory.

Required behavior:

- search text index
- apply scope filters
- exclude sensitive memory unless allowed
- exclude superseded memory unless allowed
- rank by textual match, salience, confidence, recency, and source authority
- explain ranking when requested

### consolidate

Compresses memories into ring summaries.

Required behavior:

- group by project, scope, ring, event type, and tags
- identify repeated patterns
- preserve scars and seeds
- propose heartwood candidates only with evidence
- remain idempotent for the same source set

### forget

Removes, redacts, expires, or supersedes memory.

Required behavior:

- require explicit target for destructive deletion
- support dry-run for broad queries
- update indexes
- record audit finding

### audit

Finds memory quality and safety issues.

Required behavior:

- stale memories
- contradictions
- sensitive retention violations
- low-confidence durable memories
- supersession gaps

### eval

Runs memory quality scenarios.

Required behavior:

- fixture-driven inputs
- expected recall assertions
- privacy assertions
- stale suppression assertions
- scoring report

## Recall Ranking

Default score:

```text
score =
  textual_match_weight * textual_match_score +
  salience_weight * salience +
  confidence_weight * confidence +
  recency_weight * recency_score +
  source_authority_weight * source_authority_score +
  ring_boost
```

Source authority order:

1. explicit user instruction
2. project contract or manifest
3. promoted eval result
4. direct project file reference
5. tool result
6. consolidation summary
7. inferred memory

Ring boosts:

- scars for failure, regression, bug, rollback, security, privacy, repeated mistake
- heartwood for durable preferences, project rules, promoted decisions
- seeds for planning, roadmap, alternatives, explore

## Privacy And Safety

Default policy:

- block secrets
- redact obvious credentials
- exclude sensitive memory from recall
- exclude sensitive memory from export
- shorten sensitive raw retention
- require explicit approval for health, financial, legal, and personal identifier memory

Secret detection should include:

- API key-like strings
- bearer tokens
- private keys
- password assignments
- GitHub tokens
- OpenAI-style tokens
- AWS-style access keys
- `.env`-like assignments

The framework should provide deterministic checks first. LLM-based review may be an optional adapter, never required.

## Storage Layout

Default local layout:

```text
.tree-ring/
├── config.yaml
├── memory.sqlite
├── events/
│   └── events.jsonl
├── summaries/
│   ├── daily/
│   ├── weekly/
│   ├── monthly/
│   └── yearly/
├── strata/
│   ├── heartwood.md
│   ├── scars.md
│   └── seeds.md
├── exports/
└── audit/
```

The storage location should be configurable so host frameworks can place memory in project-local, user-local, or runtime-managed directories.

## CLI Design

Command name:

```bash
tree-ring
```

Initial commands:

```bash
tree-ring init
tree-ring remember
tree-ring recall
tree-ring consolidate
tree-ring forget
tree-ring audit
tree-ring export
tree-ring import
tree-ring eval
tree-ring doctor
```

First-run CLI behavior:

- creates `.tree-ring/config.yaml`
- prints 2-3 example commands
- explains local-only default storage
- confirms that secrets are blocked by default

## Adapter Design

Adapters should not own memory semantics. They translate host-framework events into protocol events.

Adapter contract:

```python
class TreeRingAdapter:
    def capture_event(self, host_event) -> MemoryEvent | None: ...
    def recall_context(self, host_context) -> list[RecallResult]: ...
    def closeout(self, host_result) -> list[MemoryEvent]: ...
```

Adapters should be optional packages when dependencies are heavy.

Suggested package split:

- `tree-ring-memory`
- `tree-ring-memory-agentzero`
- `tree-ring-memory-langchain`
- `tree-ring-memory-mcp`

## Emotional Design Requirements

### First Encounter

Target emotion: curious and confident.

Design lever:

- The README should show a tiny complete example before architecture diagrams.
- The first sentence should say what the framework does without jargon.

### Setup

Target emotion: guided and safe.

Design lever:

- `tree-ring init` should create a sensible local config.
- The first run should explicitly say no cloud service is required.
- Secret-blocking should be visible as a default, not hidden.

### First Success

Target emotion: accomplished.

Design lever:

- The first recall should include a short "why this returned" explanation.
- Success output should include the memory id and how to delete it.

### Regular Use

Target emotion: efficient and in flow.

Design lever:

- Framework integrations should be small hooks, not a new application workflow.
- Recall should produce concise context with source links.

### Error Or Sensitive Data

Target emotion: supported, not blamed.

Design lever:

- Errors should explain what happened and what to do next.
- Sensitive data blocks should preserve a useful redacted summary when possible.

### Review And Cleanup

Target emotion: in control.

Design lever:

- Audit and export reports should make memory quality visible.
- Forget/redact flows should be explicit and reversible where possible.

## Testing Strategy

Unit tests:

- schema validation
- sensitivity detection
- storage insert/update/delete
- FTS recall
- ranking
- supersession
- consolidation idempotency
- import/export round trip

Integration tests:

- CLI init/remember/recall/forget
- SQLite migration
- adapter event translation
- eval harness scenarios

Golden fixtures:

- repeated correction becomes scar
- user preference becomes heartwood only with explicit confirmation
- stale superseded decision is hidden
- sensitive memory is excluded by default
- failed experiment surfaces for regression query

## Repository Structure

Proposed initial repository layout:

```text
Tree_Ring_Memory/
├── README.md
├── LICENSE
├── pyproject.toml
├── docs/
│   ├── protocol/
│   ├── adapters/
│   ├── evals/
│   └── feature/
├── schemas/
│   ├── memory-event.schema.json
│   ├── recall-query.schema.json
│   └── export-bundle.schema.json
├── src/
│   └── tree_ring_memory/
│       ├── __init__.py
│       ├── protocol.py
│       ├── store.py
│       ├── recall.py
│       ├── consolidate.py
│       ├── sensitivity.py
│       ├── audit.py
│       ├── export.py
│       ├── evals.py
│       └── cli.py
└── tests/
```

Implementation should not start until this design is reviewed and approved.

## Release Plan

### v0.1 Protocol Preview

- Markdown protocol docs
- JSON Schemas
- Python dataclasses or typed models
- SQLite local store
- remember/recall/forget basics
- CLI init/remember/recall
- sensitivity guard

### v0.2 Recall And Consolidation

- ranking explainability
- consolidation summaries
- audit reports
- import/export bundles
- eval fixture runner

### v0.3 Adapter Preview

- generic Python adapter
- MCP server adapter
- Agent Zero adapter extracted from the plugin work
- LangChain/LangGraph adapter prototype

### v0.4 Trust And Review

- stronger audit tooling
- optional local workbench
- memory quality score reports
- contribution guide and adapter author guide

## Open Questions

- Should the canonical protocol name use `tree-ring-memory` or `trm` in JSON envelopes?
- Should project-local manifests be part of v0.1 or held for v0.3?
- Should the first reference package avoid Pydantic to keep dependencies minimal?
- Should the sidecar daemon be a separate repo or package extra?

## Self-Review

- Placeholder scan: no TODO or TBD placeholders remain.
- Internal consistency: protocol-first design aligns with package split, storage, CLI, and release plan.
- Scope check: the first implementation remains small enough for a single v0.1 plan.
- Ambiguity check: UI/workbench and sidecar are explicitly optional later work, not v0.1 core.

