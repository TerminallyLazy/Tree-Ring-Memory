# Tree Ring Memory Framework Divergence

## HMW Question

How might we help any AI agent preserve, compress, recall, and safely forget high-value learning across workflows without tying memory to one agent framework or turning memory into a transcript dump?

## SCAMPER Options

### Option 1: Portable Memory Event Protocol

**Core idea**: Developers add Tree Ring Memory by emitting and querying a common memory event format from any agent runtime.
**Key mechanism**: A stable protocol defines memory events, scopes, rings, evidence, retention, sensitivity, recall filters, and import/export envelopes.
**Key assumption**: Agent builders will adopt a small interoperable contract if it is easier than inventing memory semantics themselves.
**SCAMPER origin**: Substitute.
**Closest competitor**: OpenTelemetry, but for agent memory rather than traces.

### Option 2: AgentOps Memory Bus

**Core idea**: Memory becomes part of the same operational stream as traces, evals, checkpoints, incidents, and tool results.
**Key mechanism**: A memory bus ingests agent events, evaluation outcomes, user corrections, and project docs, then distills them into rings.
**Key assumption**: Teams already running agent observability want durable learning attached to the evidence they trust.
**SCAMPER origin**: Combine.
**Closest competitor**: LangSmith plus custom memory pipelines.

### Option 3: Git for Agent Memory

**Core idea**: Agent memory is versioned like source code, with commits, branches, promotions, rejections, diffs, and rollbacks.
**Key mechanism**: Memory changes are append-only revisions with explicit supersession, review metadata, merge rules, and audit history.
**Key assumption**: Users and teams will trust memory more if they can inspect and revert how it changed.
**SCAMPER origin**: Adapt.
**Closest competitor**: Git, DVC, and experiment-tracking systems.

### Option 4: Evidence-First Recall Kernel

**Core idea**: The framework specializes in ranking trustworthy memories, surfacing durable lessons and scars before generic summaries.
**Key mechanism**: Recall scoring weights evidence authority, recency, confidence, salience, ring type, project scope, and sensitivity policy.
**Key assumption**: Better retrieval quality is the most valuable first wedge because existing agents already have storage options.
**SCAMPER origin**: Modify/Magnify.
**Closest competitor**: Vector-memory libraries with custom rerankers.

### Option 5: Memory Governance Layer

**Core idea**: Tree Ring Memory is adopted as the safety and governance layer for agent learning.
**Key mechanism**: Privacy policies, redaction, retention windows, contradiction audits, export controls, and human review workflows are first-class.
**Key assumption**: Organizations need agent memory they can explain, audit, and delete more than they need another recall store.
**SCAMPER origin**: Put to other use.
**Closest competitor**: Data governance platforms adapted to agent memory.

### Option 6: Headless Ring Store

**Core idea**: The first release removes dashboards and adapters, shipping only a local ring store, CLI, and typed SDK.
**Key mechanism**: A tiny embedded database and file-export layer provide remember, recall, consolidate, forget, import, and export.
**Key assumption**: The fastest adoption path is a boring, dependable library that can run anywhere.
**SCAMPER origin**: Eliminate.
**Closest competitor**: SQLite-backed app libraries.

### Option 7: Recall-First Runtime Gate

**Core idea**: Instead of agents deciding when to remember, workflows ask Tree Ring Memory what must be recalled before risky or important actions.
**Key mechanism**: Policy hooks define recall gates for project startup, user corrections, sensitive actions, deployment, reviews, and repeated failures.
**Key assumption**: Memory has the most impact when it changes agent behavior at decision points, not after-the-fact logging.
**SCAMPER origin**: Reverse.
**Closest competitor**: Policy engines and preflight check systems.

## Crazy 8s Supplements

### Option 8: Memory Sidecar Daemon

**Core idea**: Any agent integrates through a local sidecar process over HTTP, MCP, stdio, or gRPC.
**Key mechanism**: The sidecar owns storage, consolidation, privacy, and recall while SDKs stay thin.
**Key assumption**: Multi-language adoption improves when non-Python agents do not need to embed a Python library.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Local vector database sidecars and MCP servers.

### Option 9: Project Memory Manifests

**Core idea**: Projects ship human-readable memory contracts beside source code, similar to `AGENTS.md`, but standardized for portable recall.
**Key mechanism**: A manifest format defines durable project rules, scars, seeds, source authority, verification, and stale-memory markers.
**Key assumption**: File-based memory earns trust because users can review it in normal code review workflows.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: `AGENTS.md`, `CLAUDE.md`, and repo-local AI instructions.

### Option 10: Memory Eval Harness

**Core idea**: The framework includes tests that prove memory improves agent decisions instead of merely storing more text.
**Key mechanism**: Scenario fixtures assert expected recall, suppressed stale memory, surfaced scars, privacy filtering, and consolidation behavior.
**Key assumption**: Serious adopters will require measurable memory quality before trusting an agent to learn from history.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Agent evaluation frameworks and regression harnesses.

### Option 11: Local Memory Workbench

**Core idea**: Developers get a local UI to inspect rings, promote lessons, redact sensitive memories, and understand why recall happened.
**Key mechanism**: A small optional web app visualizes rings, evidence, scopes, audit findings, and import/export state.
**Key assumption**: Memory needs a visible review surface to become trusted and maintainable.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Local database browsers and agent observability dashboards.

## Curated 6

### Curated Option A: Portable Memory Event Protocol

**Representative source**: Option 1.
**Diversity test**:
- Different mechanism: Yes, a protocol contract.
- Different user behavior assumption: Yes, developers emit and consume portable events.
- Different cost/effort profile: Yes, low runtime complexity but high spec discipline.

### Curated Option B: Memory Sidecar Daemon

**Representative source**: Option 8.
**Diversity test**:
- Different mechanism: Yes, out-of-process local service.
- Different user behavior assumption: Yes, teams prefer integration by endpoint over embedding.
- Different cost/effort profile: Yes, higher operations complexity but broader language support.

### Curated Option C: Git for Agent Memory

**Representative source**: Option 3.
**Diversity test**:
- Different mechanism: Yes, versioned memory revisions and review workflows.
- Different user behavior assumption: Yes, users want inspectability and rollback.
- Different cost/effort profile: Yes, medium implementation cost with strong trust payoff.

### Curated Option D: Evidence-First Recall Kernel

**Representative source**: Option 4.
**Diversity test**:
- Different mechanism: Yes, retrieval and ranking core.
- Different user behavior assumption: Yes, adopters already have stores but need better decision support.
- Different cost/effort profile: Yes, algorithmic work before broader product surface.

### Curated Option E: Recall-First Runtime Gate

**Representative source**: Option 7.
**Diversity test**:
- Different mechanism: Yes, policy gates before agent actions.
- Different user behavior assumption: Yes, memory should interrupt or guide risky workflows.
- Different cost/effort profile: Yes, integration-heavy but behaviorally powerful.

### Curated Option F: Memory Eval Harness

**Representative source**: Option 10.
**Diversity test**:
- Different mechanism: Yes, quality tests and scenario fixtures.
- Different user behavior assumption: Yes, adopters need proof that memory changes outcomes.
- Different cost/effort profile: Yes, lower runtime scope but high adoption credibility.

## Eliminated Or Merged Options

- **AgentOps Memory Bus** was merged into the protocol and sidecar directions. It is valuable as an integration story, but not distinct enough as the first framework shape.
- **Memory Governance Layer** remains a required capability across all directions, but as a standalone option it narrows the audience too early.
- **Headless Ring Store** was merged into Portable Memory Event Protocol as the first reference implementation style.
- **Project Memory Manifests** remains a future adapter surface for DOX-like repo context, but should not define the whole framework.
- **Local Memory Workbench** remains important for usability and trust, but should be optional so the open-source core stays framework-agnostic.

## Emotional Design Notes

### First Encounter

**Target emotion**: Curious and confident.
**Design lever**: Start with a clear promise: "portable memory that agents can explain, test, and forget." Avoid presenting the framework as another vector database.

### Setup

**Target emotion**: Guided and safe.
**Design lever**: Provide a two-minute path: install, initialize local store, emit one memory, recall it, inspect it, forget it. Defaults should block obvious secrets and avoid cloud services.

### First Success

**Target emotion**: Accomplished.
**Design lever**: The first recall should explain why it was returned, where it came from, and how to delete or promote it.

### Regular Use

**Target emotion**: Efficient and in flow.
**Design lever**: Integrations should feel like small hooks around existing workflows rather than a new operational burden.

### Error Or Sensitive Data

**Target emotion**: Supported, not blamed.
**Design lever**: Error copy should say what was blocked or redacted and what action is available next. Sensitive data should fail closed by default.

### Completion Or Review

**Target emotion**: In control.
**Design lever**: Export, audit, and memory eval reports should make memory quality visible without forcing users to read raw transcripts.

