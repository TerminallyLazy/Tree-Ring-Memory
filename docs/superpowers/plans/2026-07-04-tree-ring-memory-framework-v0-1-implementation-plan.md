# Tree Ring Memory Framework v0.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first framework-agnostic Tree Ring Memory release: a protocol preview with a working Python reference library, local SQLite storage, privacy guard, recall, forget, and CLI.

**Architecture:** The framework starts as a protocol-first Python package. Protocol dataclasses and JSON Schemas define portable memory envelopes, while the reference implementation stores events in SQLite with FTS and JSONL export-friendly shapes. The CLI exercises the same public API that adapters will use.

**Tech Stack:** Python 3.11+, stdlib dataclasses, sqlite3, argparse, json, pathlib, pytest for tests, Markdown docs, JSON Schema files.

---

## Scope Check

This plan implements the v0.1 Protocol Preview only.

Included:

- repository metadata
- protocol documentation
- JSON Schemas
- typed Python models
- deterministic sensitivity guard
- SQLite + FTS local store
- remember, recall, forget
- basic CLI
- tests

Deferred to later plans:

- consolidation engine
- eval harness
- import/export bundles
- Agent Zero adapter
- LangChain/LangGraph adapter
- MCP server adapter
- sidecar daemon
- optional local workbench

## File Structure

- Create `README.md`: public-facing promise, install-from-source instructions, first example.
- Create `LICENSE`: open-source license text selected by project owner during implementation; use MIT in v0.1 unless changed before Task 1.
- Create `pyproject.toml`: package metadata and pytest config.
- Create `docs/protocol/memory-event.md`: protocol explanation.
- Create `schemas/memory-event.schema.json`: portable event schema.
- Create `schemas/recall-query.schema.json`: recall request schema.
- Create `schemas/recall-result.schema.json`: recall response schema.
- Create `src/tree_ring_memory/__init__.py`: public exports.
- Create `src/tree_ring_memory/models.py`: dataclasses, enums, validation helpers.
- Create `src/tree_ring_memory/sensitivity.py`: deterministic secret/sensitive detection.
- Create `src/tree_ring_memory/store.py`: SQLite store, migrations, FTS, remember/get/list/delete/update.
- Create `src/tree_ring_memory/recall.py`: recall query, filters, ranking.
- Create `src/tree_ring_memory/api.py`: `TreeRingMemory` facade.
- Create `src/tree_ring_memory/cli.py`: command-line interface.
- Create `tests/test_models.py`: protocol object tests.
- Create `tests/test_sensitivity.py`: secret and sensitive detection tests.
- Create `tests/test_store.py`: SQLite storage tests.
- Create `tests/test_recall.py`: ranking/filter tests.
- Create `tests/test_cli.py`: CLI smoke tests.

---

### Task 1: Repository Bootstrap

**Files:**
- Create: `README.md`
- Create: `LICENSE`
- Create: `pyproject.toml`
- Create: `.gitignore`

- [ ] **Step 1: Create package metadata**

Create `pyproject.toml`:

```toml
[build-system]
requires = ["hatchling>=1.25"]
build-backend = "hatchling.build"

[project]
name = "tree-ring-memory"
version = "0.1.0"
description = "Framework-agnostic tree-ring memory for AI agents."
readme = "README.md"
requires-python = ">=3.11"
license = { file = "LICENSE" }
authors = [
  { name = "TerminallyLazy" }
]
keywords = ["ai", "agents", "memory", "sqlite", "recall", "privacy"]
classifiers = [
  "Development Status :: 3 - Alpha",
  "Intended Audience :: Developers",
  "License :: OSI Approved :: MIT License",
  "Programming Language :: Python :: 3",
  "Programming Language :: Python :: 3.11",
  "Programming Language :: Python :: 3.12",
  "Topic :: Software Development :: Libraries :: Python Modules"
]
dependencies = []

[project.optional-dependencies]
dev = ["pytest>=8.0"]

[project.scripts]
tree-ring = "tree_ring_memory.cli:main"

[tool.pytest.ini_options]
testpaths = ["tests"]
pythonpath = ["src"]
addopts = "-q"
```

- [ ] **Step 2: Create `.gitignore`**

Create `.gitignore`:

```gitignore
__pycache__/
*.py[cod]
.pytest_cache/
.ruff_cache/
.mypy_cache/
.venv/
dist/
build/
*.egg-info/
.tree-ring/
```

- [ ] **Step 3: Create initial README**

Create `README.md`:

```markdown
# Tree Ring Memory

Tree Ring Memory is a framework-agnostic memory lifecycle layer for AI agents.

It helps agents remember useful decisions, warnings, preferences, and lessons without turning memory into a transcript dump. Fresh memory stays detailed, older memory compresses into rings, important scars remain visible, and durable truths become heartwood.

## v0.1 Status

This repository is in protocol-preview status. The first implementation target is a local Python reference library with SQLite storage and no required cloud services.

## First Example

```python
from tree_ring_memory import TreeRingMemory

memory = TreeRingMemory.open(".tree-ring")
event = memory.remember(
    summary="Use Store Gate before reading Agent Zero frontend stores.",
    event_type="lesson",
    scope="project",
    project="agent-zero",
    tags=["frontend", "agent-zero"],
)

results = memory.recall("frontend store initialization", project="agent-zero")
for result in results:
    print(result.memory.summary, result.score)
```

## Design Docs

- `docs/superpowers/specs/2026-07-04-tree-ring-memory-framework-design.md`
- `docs/feature/tree-ring-memory-framework/diverge/options-raw.md`

## Principles

- Local-first by default.
- Protocol before adapters.
- Explainable recall.
- Sensitive data fails closed.
- Forgetting and supersession are first-class.
- Memory quality should be testable.
```

- [ ] **Step 4: Create MIT license**

Create `LICENSE`:

```text
MIT License

Copyright (c) 2026 TerminallyLazy

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 5: Verify bootstrap**

Run:

```bash
python -m pip install -e ".[dev]"
pytest
```

Expected:

```text
no tests ran
```

If pytest exits nonzero because there are no tests, run:

```bash
python -m pip install -e ".[dev]"
```

Expected: package installs successfully.

- [ ] **Step 6: Commit**

```bash
git add README.md LICENSE pyproject.toml .gitignore
git commit -m "chore: bootstrap tree ring memory package"
```

---

### Task 2: Protocol Models

**Files:**
- Create: `src/tree_ring_memory/__init__.py`
- Create: `src/tree_ring_memory/models.py`
- Create: `tests/test_models.py`

- [ ] **Step 1: Write model tests**

Create `tests/test_models.py`:

```python
from datetime import UTC, datetime

import pytest

from tree_ring_memory.models import MemoryEvent, MemorySource, ValidationError


def test_valid_memory_event_round_trips_to_dict():
    event = MemoryEvent.new(
        summary="Prefer local SQLite storage for v0.1.",
        event_type="decision",
        scope="project",
        ring="heartwood",
        project="tree-ring-memory",
        source=MemorySource(type="manual", ref="test"),
        tags=["sqlite", "v0.1"],
        salience=0.8,
        confidence=0.9,
    )

    payload = event.to_dict()

    assert payload["id"].startswith("mem_")
    assert payload["summary"] == "Prefer local SQLite storage for v0.1."
    assert payload["ring"] == "heartwood"
    assert payload["source"]["ref"] == "test"
    assert payload["tags"] == ["sqlite", "v0.1"]


def test_missing_summary_is_rejected():
    with pytest.raises(ValidationError, match="summary is required"):
        MemoryEvent.new(summary="", event_type="decision")


def test_invalid_ring_is_rejected():
    with pytest.raises(ValidationError, match="invalid ring"):
        MemoryEvent.new(summary="Bad ring.", event_type="decision", ring="bark")


def test_invalid_score_is_rejected():
    with pytest.raises(ValidationError, match="salience"):
        MemoryEvent.new(summary="Bad score.", event_type="decision", salience=2.0)


def test_from_dict_preserves_created_at():
    created_at = datetime(2026, 7, 4, 18, 0, tzinfo=UTC).isoformat()
    event = MemoryEvent.from_dict({
        "id": "mem_fixed",
        "created_at": created_at,
        "updated_at": created_at,
        "project": "demo",
        "agent_profile": None,
        "scope": "project",
        "ring": "outer",
        "event_type": "lesson",
        "summary": "A preserved source ref matters.",
        "details": "",
        "source": {"type": "file", "ref": "README.md", "quote": ""},
        "tags": ["docs"],
        "salience": 0.5,
        "confidence": 0.6,
        "sensitivity": "normal",
        "retention": "normal",
        "expires_at": None,
        "supersedes": [],
        "superseded_by": None,
        "links": [],
        "review": {"needs_review": False, "review_reason": None, "reviewed_at": None, "reviewed_by": None},
    })

    assert event.created_at.isoformat() == created_at
    assert event.source.ref == "README.md"
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
pytest tests/test_models.py -q
```

Expected: import failure for `tree_ring_memory.models`.

- [ ] **Step 3: Implement models**

Create `src/tree_ring_memory/models.py`:

```python
from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from itertools import count
from typing import Any


RINGS = {"cambium", "outer", "inner", "heartwood", "scar", "seed"}
SCOPES = {"global", "project", "agent", "session", "workflow", "tool", "eval", "manual"}
SENSITIVITY = {"normal", "private", "secret", "health", "financial", "legal", "personal_identifier"}
RETENTION = {"ephemeral", "normal", "durable", "user_pinned", "forget_after_date"}

_ID_COUNTER = count(1)


class ValidationError(ValueError):
    """Raised when a memory protocol object is invalid."""


def now_utc() -> datetime:
    return datetime.now(UTC)


def parse_datetime(value: str | None, field_name: str) -> datetime | None:
    if value is None:
        return None
    try:
        return datetime.fromisoformat(value)
    except ValueError as exc:
        raise ValidationError(f"{field_name} must be ISO-8601 datetime") from exc


def _validate_score(name: str, value: float) -> float:
    number = float(value)
    if number < 0 or number > 1:
        raise ValidationError(f"{name} must be between 0 and 1")
    return number


@dataclass(slots=True)
class MemorySource:
    type: str = "manual"
    ref: str = ""
    quote: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {"type": self.type, "ref": self.ref, "quote": self.quote}

    @classmethod
    def from_dict(cls, value: dict[str, Any] | None) -> "MemorySource":
        value = value or {}
        return cls(
            type=str(value.get("type") or "manual"),
            ref=str(value.get("ref") or ""),
            quote=str(value.get("quote") or ""),
        )


@dataclass(slots=True)
class MemoryLink:
    type: str
    target: str

    def to_dict(self) -> dict[str, str]:
        return {"type": self.type, "target": self.target}

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "MemoryLink":
        return cls(type=str(value.get("type") or ""), target=str(value.get("target") or ""))


@dataclass(slots=True)
class MemoryReview:
    needs_review: bool = False
    review_reason: str | None = None
    reviewed_at: str | None = None
    reviewed_by: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "needs_review": self.needs_review,
            "review_reason": self.review_reason,
            "reviewed_at": self.reviewed_at,
            "reviewed_by": self.reviewed_by,
        }

    @classmethod
    def from_dict(cls, value: dict[str, Any] | None) -> "MemoryReview":
        value = value or {}
        return cls(
            needs_review=bool(value.get("needs_review", False)),
            review_reason=value.get("review_reason"),
            reviewed_at=value.get("reviewed_at"),
            reviewed_by=value.get("reviewed_by"),
        )


@dataclass(slots=True)
class MemoryEvent:
    id: str
    created_at: datetime
    updated_at: datetime
    scope: str
    ring: str
    event_type: str
    summary: str
    project: str | None = None
    agent_profile: str | None = None
    details: str = ""
    source: MemorySource = field(default_factory=MemorySource)
    tags: list[str] = field(default_factory=list)
    salience: float = 0.5
    confidence: float = 0.5
    sensitivity: str = "normal"
    retention: str = "normal"
    expires_at: datetime | None = None
    supersedes: list[str] = field(default_factory=list)
    superseded_by: str | None = None
    links: list[MemoryLink] = field(default_factory=list)
    review: MemoryReview = field(default_factory=MemoryReview)

    @classmethod
    def new(
        cls,
        *,
        summary: str,
        event_type: str,
        scope: str = "global",
        ring: str = "cambium",
        project: str | None = None,
        agent_profile: str | None = None,
        details: str = "",
        source: MemorySource | None = None,
        tags: list[str] | None = None,
        salience: float = 0.5,
        confidence: float = 0.5,
        sensitivity: str = "normal",
        retention: str = "normal",
        expires_at: datetime | None = None,
        supersedes: list[str] | None = None,
        links: list[MemoryLink] | None = None,
        review: MemoryReview | None = None,
    ) -> "MemoryEvent":
        timestamp = now_utc()
        event = cls(
            id=f"mem_{timestamp.strftime('%Y%m%d_%H%M%S')}_{next(_ID_COUNTER):06d}",
            created_at=timestamp,
            updated_at=timestamp,
            project=project,
            agent_profile=agent_profile,
            scope=scope,
            ring=ring,
            event_type=event_type,
            summary=summary,
            details=details,
            source=source or MemorySource(),
            tags=tags or [],
            salience=salience,
            confidence=confidence,
            sensitivity=sensitivity,
            retention=retention,
            expires_at=expires_at,
            supersedes=supersedes or [],
            links=links or [],
            review=review or MemoryReview(),
        )
        event.validate()
        return event

    def validate(self) -> None:
        if not self.summary.strip():
            raise ValidationError("summary is required")
        if not self.event_type.strip():
            raise ValidationError("event_type is required")
        if self.scope not in SCOPES:
            raise ValidationError(f"invalid scope: {self.scope}")
        if self.ring not in RINGS:
            raise ValidationError(f"invalid ring: {self.ring}")
        if self.sensitivity not in SENSITIVITY:
            raise ValidationError(f"invalid sensitivity: {self.sensitivity}")
        if self.retention not in RETENTION:
            raise ValidationError(f"invalid retention: {self.retention}")
        self.salience = _validate_score("salience", self.salience)
        self.confidence = _validate_score("confidence", self.confidence)

    def to_dict(self) -> dict[str, Any]:
        self.validate()
        return {
            "id": self.id,
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
            "project": self.project,
            "agent_profile": self.agent_profile,
            "scope": self.scope,
            "ring": self.ring,
            "event_type": self.event_type,
            "summary": self.summary,
            "details": self.details,
            "source": self.source.to_dict(),
            "tags": list(self.tags),
            "salience": self.salience,
            "confidence": self.confidence,
            "sensitivity": self.sensitivity,
            "retention": self.retention,
            "expires_at": self.expires_at.isoformat() if self.expires_at else None,
            "supersedes": list(self.supersedes),
            "superseded_by": self.superseded_by,
            "links": [link.to_dict() for link in self.links],
            "review": self.review.to_dict(),
        }

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "MemoryEvent":
        event = cls(
            id=str(value.get("id") or ""),
            created_at=parse_datetime(value.get("created_at"), "created_at") or now_utc(),
            updated_at=parse_datetime(value.get("updated_at"), "updated_at") or now_utc(),
            project=value.get("project"),
            agent_profile=value.get("agent_profile"),
            scope=str(value.get("scope") or "global"),
            ring=str(value.get("ring") or "cambium"),
            event_type=str(value.get("event_type") or ""),
            summary=str(value.get("summary") or ""),
            details=str(value.get("details") or ""),
            source=MemorySource.from_dict(value.get("source")),
            tags=[str(tag) for tag in value.get("tags") or []],
            salience=float(value.get("salience", 0.5)),
            confidence=float(value.get("confidence", 0.5)),
            sensitivity=str(value.get("sensitivity") or "normal"),
            retention=str(value.get("retention") or "normal"),
            expires_at=parse_datetime(value.get("expires_at"), "expires_at"),
            supersedes=[str(item) for item in value.get("supersedes") or []],
            superseded_by=value.get("superseded_by"),
            links=[MemoryLink.from_dict(link) for link in value.get("links") or []],
            review=MemoryReview.from_dict(value.get("review")),
        )
        event.validate()
        return event
```

Create `src/tree_ring_memory/__init__.py`:

```python
from tree_ring_memory.api import TreeRingMemory
from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource, ValidationError

__all__ = [
    "TreeRingMemory",
    "MemoryEvent",
    "MemoryLink",
    "MemoryReview",
    "MemorySource",
    "ValidationError",
]
```

- [ ] **Step 4: Run tests**

Run:

```bash
pytest tests/test_models.py -q
```

Expected:

```text
5 passed
```

- [ ] **Step 5: Commit**

```bash
git add src/tree_ring_memory/__init__.py src/tree_ring_memory/models.py tests/test_models.py
git commit -m "feat: add memory protocol models"
```

---

### Task 3: Protocol JSON Schemas And Docs

**Files:**
- Create: `schemas/memory-event.schema.json`
- Create: `schemas/recall-query.schema.json`
- Create: `schemas/recall-result.schema.json`
- Create: `docs/protocol/memory-event.md`

- [ ] **Step 1: Create memory event schema**

Create `schemas/memory-event.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://tree-ring-memory.dev/schemas/memory-event.schema.json",
  "title": "Tree Ring Memory Event",
  "type": "object",
  "required": ["id", "created_at", "updated_at", "scope", "ring", "event_type", "summary", "source", "tags", "salience", "confidence", "sensitivity", "retention"],
  "properties": {
    "id": { "type": "string", "minLength": 1 },
    "created_at": { "type": "string", "format": "date-time" },
    "updated_at": { "type": "string", "format": "date-time" },
    "project": { "type": ["string", "null"] },
    "agent_profile": { "type": ["string", "null"] },
    "scope": { "enum": ["global", "project", "agent", "session", "workflow", "tool", "eval", "manual"] },
    "ring": { "enum": ["cambium", "outer", "inner", "heartwood", "scar", "seed"] },
    "event_type": { "type": "string", "minLength": 1 },
    "summary": { "type": "string", "minLength": 1 },
    "details": { "type": "string" },
    "source": {
      "type": "object",
      "required": ["type"],
      "properties": {
        "type": { "type": "string", "minLength": 1 },
        "ref": { "type": "string" },
        "quote": { "type": "string" }
      },
      "additionalProperties": false
    },
    "tags": { "type": "array", "items": { "type": "string" } },
    "salience": { "type": "number", "minimum": 0, "maximum": 1 },
    "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
    "sensitivity": { "enum": ["normal", "private", "secret", "health", "financial", "legal", "personal_identifier"] },
    "retention": { "enum": ["ephemeral", "normal", "durable", "user_pinned", "forget_after_date"] },
    "expires_at": { "type": ["string", "null"], "format": "date-time" },
    "supersedes": { "type": "array", "items": { "type": "string" } },
    "superseded_by": { "type": ["string", "null"] },
    "links": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["type", "target"],
        "properties": {
          "type": { "type": "string" },
          "target": { "type": "string" }
        },
        "additionalProperties": false
      }
    },
    "review": {
      "type": "object",
      "required": ["needs_review"],
      "properties": {
        "needs_review": { "type": "boolean" },
        "review_reason": { "type": ["string", "null"] },
        "reviewed_at": { "type": ["string", "null"] },
        "reviewed_by": { "type": ["string", "null"] }
      },
      "additionalProperties": false
    }
  },
  "additionalProperties": false
}
```

- [ ] **Step 2: Create recall schemas**

Create `schemas/recall-query.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://tree-ring-memory.dev/schemas/recall-query.schema.json",
  "title": "Tree Ring Recall Query",
  "type": "object",
  "required": ["query"],
  "properties": {
    "query": { "type": "string" },
    "project": { "type": ["string", "null"] },
    "agent_profile": { "type": ["string", "null"] },
    "scope": { "type": ["string", "null"] },
    "rings": { "type": "array", "items": { "type": "string" } },
    "event_types": { "type": "array", "items": { "type": "string" } },
    "include_sensitive": { "type": "boolean", "default": false },
    "include_superseded": { "type": "boolean", "default": false },
    "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 8 },
    "explain_ranking": { "type": "boolean", "default": false }
  },
  "additionalProperties": false
}
```

Create `schemas/recall-result.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://tree-ring-memory.dev/schemas/recall-result.schema.json",
  "title": "Tree Ring Recall Result",
  "type": "object",
  "required": ["memory", "score"],
  "properties": {
    "memory": { "$ref": "memory-event.schema.json" },
    "score": { "type": "number" },
    "ranking": {
      "type": "object",
      "properties": {
        "textual_match": { "type": "number" },
        "salience": { "type": "number" },
        "confidence": { "type": "number" },
        "recency": { "type": "number" },
        "source_authority": { "type": "number" },
        "ring_boost": { "type": "number" }
      },
      "additionalProperties": false
    }
  },
  "additionalProperties": false
}
```

- [ ] **Step 3: Create protocol docs**

Create `docs/protocol/memory-event.md`:

```markdown
# Memory Event Protocol

`MemoryEvent` is the portable unit of Tree Ring Memory.

The event is not a transcript line. It is a meaningful memory statement with scope, ring, source evidence, confidence, salience, sensitivity, retention, and review state.

## Rings

- `cambium`: fresh active memory
- `outer`: recent summarized memory
- `inner`: older compressed memory
- `heartwood`: durable truths
- `scar`: important negative lessons
- `seed`: unresolved future possibilities

## Recall Defaults

Recall excludes sensitive and superseded memory unless explicitly requested. Results should include source evidence and ranking explanation when `explain_ranking` is true.

## Privacy Defaults

Secrets are blocked by default. Sensitive memory is excluded from recall and export by default.
```

- [ ] **Step 4: Verify JSON syntax**

Run:

```bash
python -m json.tool schemas/memory-event.schema.json >/dev/null
python -m json.tool schemas/recall-query.schema.json >/dev/null
python -m json.tool schemas/recall-result.schema.json >/dev/null
```

Expected: all commands exit with code 0.

- [ ] **Step 5: Commit**

```bash
git add schemas docs/protocol
git commit -m "docs: add portable memory protocol schemas"
```

---

### Task 4: Sensitivity Guard

**Files:**
- Create: `src/tree_ring_memory/sensitivity.py`
- Create: `tests/test_sensitivity.py`

- [ ] **Step 1: Write sensitivity tests**

Create `tests/test_sensitivity.py`:

```python
import pytest

from tree_ring_memory.sensitivity import SensitivityGuard, SensitiveMemoryBlocked


def test_blocks_openai_style_secret():
    guard = SensitivityGuard()

    with pytest.raises(SensitiveMemoryBlocked):
        guard.check_or_raise("Use key sk-proj-abcdefghijklmnopqrstuvwxyz1234567890")


def test_redacts_password_assignment():
    guard = SensitivityGuard(block_secret_storage=False)

    result = guard.inspect("password = hunter2")

    assert result.sensitivity == "secret"
    assert "hunter2" not in result.redacted_text
    assert "[REDACTED_SECRET]" in result.redacted_text


def test_detects_health_text_as_sensitive():
    guard = SensitivityGuard()

    result = guard.inspect("User mentioned a private diagnosis during setup.")

    assert result.sensitivity == "health"
    assert result.requires_explicit_approval is True


def test_normal_text_passes():
    guard = SensitivityGuard()

    result = guard.inspect("Use SQLite for local storage.")

    assert result.sensitivity == "normal"
    assert result.redacted_text == "Use SQLite for local storage."
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
pytest tests/test_sensitivity.py -q
```

Expected: import failure for `tree_ring_memory.sensitivity`.

- [ ] **Step 3: Implement sensitivity guard**

Create `src/tree_ring_memory/sensitivity.py`:

```python
from __future__ import annotations

from dataclasses import dataclass
import re


class SensitiveMemoryBlocked(ValueError):
    """Raised when policy blocks storing sensitive memory."""


@dataclass(slots=True)
class SensitivityResult:
    sensitivity: str
    redacted_text: str
    findings: list[str]
    requires_explicit_approval: bool = False


class SensitivityGuard:
    SECRET_PATTERNS = [
        ("openai_key", re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b")),
        ("github_token", re.compile(r"\bgh[pousr]_[A-Za-z0-9_]{20,}\b")),
        ("aws_access_key", re.compile(r"\bAKIA[0-9A-Z]{16}\b")),
        ("bearer_token", re.compile(r"\bBearer\s+[A-Za-z0-9._~+/=-]{20,}\b", re.IGNORECASE)),
        ("private_key", re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----")),
        ("password_assignment", re.compile(r"(?i)\b(password|passwd|pwd|secret|token)\s*=\s*[^\\s]+")),
    ]
    CATEGORY_PATTERNS = [
        ("health", re.compile(r"(?i)\b(diagnosis|diagnosed|medical|medication|prescription|therapy|hospital)\b")),
        ("financial", re.compile(r"(?i)\b(bank account|routing number|credit card|tax return|paystub|salary)\b")),
        ("legal", re.compile(r"(?i)\b(lawsuit|attorney|legal advice|court order|subpoena|contract dispute)\b")),
        ("personal_identifier", re.compile(r"(?i)\b(ssn|social security|passport|driver'?s license)\b")),
    ]

    def __init__(self, *, block_secret_storage: bool = True) -> None:
        self.block_secret_storage = block_secret_storage

    def inspect(self, text: str) -> SensitivityResult:
        findings: list[str] = []
        redacted = text
        for name, pattern in self.SECRET_PATTERNS:
            if pattern.search(redacted):
                findings.append(name)
                redacted = pattern.sub("[REDACTED_SECRET]", redacted)
        if findings:
            return SensitivityResult("secret", redacted, findings)
        for category, pattern in self.CATEGORY_PATTERNS:
            if pattern.search(text):
                return SensitivityResult(category, text, [category], requires_explicit_approval=True)
        return SensitivityResult("normal", text, [])

    def check_or_raise(self, text: str) -> SensitivityResult:
        result = self.inspect(text)
        if result.sensitivity == "secret" and self.block_secret_storage:
            raise SensitiveMemoryBlocked("secret-like memory is blocked by policy")
        return result
```

- [ ] **Step 4: Run sensitivity tests**

Run:

```bash
pytest tests/test_sensitivity.py -q
```

Expected:

```text
4 passed
```

- [ ] **Step 5: Commit**

```bash
git add src/tree_ring_memory/sensitivity.py tests/test_sensitivity.py
git commit -m "feat: add deterministic sensitivity guard"
```

---

### Task 5: SQLite Store

**Files:**
- Create: `src/tree_ring_memory/store.py`
- Create: `tests/test_store.py`

- [ ] **Step 1: Write store tests**

Create `tests/test_store.py`:

```python
from tree_ring_memory.models import MemoryEvent, MemorySource
from tree_ring_memory.store import SQLiteMemoryStore


def test_store_inserts_and_gets_memory(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    event = MemoryEvent.new(
        summary="SQLite stores portable memory.",
        event_type="lesson",
        scope="project",
        project="demo",
        source=MemorySource(type="manual", ref="test"),
    )

    store.put(event)
    loaded = store.get(event.id)

    assert loaded is not None
    assert loaded.summary == "SQLite stores portable memory."
    assert loaded.source.ref == "test"


def test_store_searches_fts(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="Avoid stale cache without invalidation.", event_type="warning", ring="scar"))
    store.put(MemoryEvent.new(summary="Use local SQLite for v0.1.", event_type="decision", ring="heartwood"))

    results = store.search_text("stale cache")

    assert [event.ring for event in results] == ["scar"]


def test_supersede_hides_old_memory_by_default(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    old = MemoryEvent.new(summary="Use polling.", event_type="decision")
    new = MemoryEvent.new(summary="Use snapshot invalidation.", event_type="decision", supersedes=[old.id])
    store.put(old)
    store.put(new)
    store.supersede(old.id, new.id)

    visible = store.list_all(include_superseded=False)

    assert [event.id for event in visible] == [new.id]


def test_delete_removes_fts_row(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    event = MemoryEvent.new(summary="Delete me from search.", event_type="lesson")
    store.put(event)

    store.delete(event.id)

    assert store.get(event.id) is None
    assert store.search_text("Delete me") == []
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
pytest tests/test_store.py -q
```

Expected: import failure for `tree_ring_memory.store`.

- [ ] **Step 3: Implement SQLite store**

Create `src/tree_ring_memory/store.py`:

```python
from __future__ import annotations

import json
from pathlib import Path
import sqlite3
from typing import Iterable

from tree_ring_memory.models import MemoryEvent


class SQLiteMemoryStore:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection
        self.connection.row_factory = sqlite3.Row
        self.migrate()

    @classmethod
    def open(cls, path: str | Path) -> "SQLiteMemoryStore":
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(path)
        return cls(connection)

    def migrate(self) -> None:
        self.connection.executescript(
            """
            CREATE TABLE IF NOT EXISTS memories (
              id TEXT PRIMARY KEY,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              project TEXT,
              agent_profile TEXT,
              scope TEXT NOT NULL,
              ring TEXT NOT NULL,
              event_type TEXT NOT NULL,
              summary TEXT NOT NULL,
              details TEXT NOT NULL,
              source_json TEXT NOT NULL,
              tags_json TEXT NOT NULL,
              salience REAL NOT NULL,
              confidence REAL NOT NULL,
              sensitivity TEXT NOT NULL,
              retention TEXT NOT NULL,
              expires_at TEXT,
              supersedes_json TEXT NOT NULL,
              superseded_by TEXT,
              links_json TEXT NOT NULL,
              review_json TEXT NOT NULL,
              raw_json TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
              id UNINDEXED,
              summary,
              details,
              tags,
              source_ref
            );
            """
        )
        self.connection.commit()

    def put(self, event: MemoryEvent) -> None:
        payload = event.to_dict()
        source = payload["source"]
        with self.connection:
            self.connection.execute(
                """
                INSERT OR REPLACE INTO memories (
                  id, created_at, updated_at, project, agent_profile, scope, ring,
                  event_type, summary, details, source_json, tags_json, salience,
                  confidence, sensitivity, retention, expires_at, supersedes_json,
                  superseded_by, links_json, review_json, raw_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    event.id,
                    payload["created_at"],
                    payload["updated_at"],
                    event.project,
                    event.agent_profile,
                    event.scope,
                    event.ring,
                    event.event_type,
                    event.summary,
                    event.details,
                    json.dumps(source, sort_keys=True),
                    json.dumps(event.tags, sort_keys=True),
                    event.salience,
                    event.confidence,
                    event.sensitivity,
                    event.retention,
                    payload["expires_at"],
                    json.dumps(event.supersedes, sort_keys=True),
                    event.superseded_by,
                    json.dumps([link.to_dict() for link in event.links], sort_keys=True),
                    json.dumps(event.review.to_dict(), sort_keys=True),
                    json.dumps(payload, sort_keys=True),
                ),
            )
            self.connection.execute("DELETE FROM memory_fts WHERE id = ?", (event.id,))
            self.connection.execute(
                "INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)",
                (event.id, event.summary, event.details, " ".join(event.tags), source.get("ref", "")),
            )

    def get(self, memory_id: str) -> MemoryEvent | None:
        row = self.connection.execute("SELECT raw_json FROM memories WHERE id = ?", (memory_id,)).fetchone()
        if row is None:
            return None
        return MemoryEvent.from_dict(json.loads(row["raw_json"]))

    def list_all(self, *, include_superseded: bool = False) -> list[MemoryEvent]:
        sql = "SELECT raw_json FROM memories"
        if not include_superseded:
            sql += " WHERE superseded_by IS NULL"
        sql += " ORDER BY created_at DESC"
        rows = self.connection.execute(sql).fetchall()
        return [MemoryEvent.from_dict(json.loads(row["raw_json"])) for row in rows]

    def search_text(self, query: str, *, include_superseded: bool = False) -> list[MemoryEvent]:
        if not query.strip():
            return self.list_all(include_superseded=include_superseded)
        rows = self.connection.execute(
            """
            SELECT memories.raw_json
            FROM memory_fts
            JOIN memories ON memories.id = memory_fts.id
            WHERE memory_fts MATCH ?
            ORDER BY rank
            """,
            (query,),
        ).fetchall()
        events = [MemoryEvent.from_dict(json.loads(row["raw_json"])) for row in rows]
        if include_superseded:
            return events
        return [event for event in events if event.superseded_by is None]

    def supersede(self, old_id: str, new_id: str) -> None:
        old = self.get(old_id)
        if old is None:
            return
        old.superseded_by = new_id
        with self.connection:
            self.connection.execute(
                "UPDATE memories SET superseded_by = ?, raw_json = ? WHERE id = ?",
                (new_id, json.dumps(old.to_dict(), sort_keys=True), old_id),
            )

    def delete(self, memory_id: str) -> None:
        with self.connection:
            self.connection.execute("DELETE FROM memories WHERE id = ?", (memory_id,))
            self.connection.execute("DELETE FROM memory_fts WHERE id = ?", (memory_id,))

    def close(self) -> None:
        self.connection.close()
```

- [ ] **Step 4: Run store tests**

Run:

```bash
pytest tests/test_store.py -q
```

Expected:

```text
4 passed
```

- [ ] **Step 5: Commit**

```bash
git add src/tree_ring_memory/store.py tests/test_store.py
git commit -m "feat: add sqlite memory store"
```

---

### Task 6: Recall Ranking

**Files:**
- Create: `src/tree_ring_memory/recall.py`
- Create: `tests/test_recall.py`

- [ ] **Step 1: Write recall tests**

Create `tests/test_recall.py`:

```python
from tree_ring_memory.models import MemoryEvent, MemorySource
from tree_ring_memory.recall import MemoryRetriever
from tree_ring_memory.store import SQLiteMemoryStore


def test_recall_excludes_sensitive_by_default(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="Normal memory", event_type="lesson"))
    store.put(MemoryEvent.new(summary="Private bank account note", event_type="lesson", sensitivity="financial"))

    results = MemoryRetriever(store).recall("memory")

    assert [result.memory.summary for result in results] == ["Normal memory"]


def test_scar_boost_for_failure_query(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="Avoid stale frontend cache.", event_type="warning", ring="scar", confidence=0.7))
    store.put(MemoryEvent.new(summary="Use cache for fast reads.", event_type="lesson", ring="outer", confidence=0.9))

    results = MemoryRetriever(store).recall("failure stale cache")

    assert results[0].memory.ring == "scar"


def test_project_filter_limits_results(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="Use SQLite.", event_type="decision", project="a"))
    store.put(MemoryEvent.new(summary="Use Postgres.", event_type="decision", project="b"))

    results = MemoryRetriever(store).recall("Use", project="b")

    assert [result.memory.project for result in results] == ["b"]


def test_explain_ranking_returns_factors(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="User prefers local memory.", event_type="preference", ring="heartwood"))

    result = MemoryRetriever(store).recall("local memory", explain_ranking=True)[0]

    assert result.ranking["confidence"] >= 0
    assert result.score > 0
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
pytest tests/test_recall.py -q
```

Expected: import failure for `tree_ring_memory.recall`.

- [ ] **Step 3: Implement retriever**

Create `src/tree_ring_memory/recall.py`:

```python
from __future__ import annotations

from dataclasses import dataclass
from datetime import UTC, datetime
from math import exp

from tree_ring_memory.models import MemoryEvent
from tree_ring_memory.store import SQLiteMemoryStore


FAILURE_TERMS = {"error", "failure", "regression", "bug", "rejected", "rollback", "stale", "conflict", "security", "privacy", "mistake"}
HEARTWOOD_TERMS = {"preference", "rule", "constraint", "decision", "durable"}
SEED_TERMS = {"planning", "roadmap", "future", "alternative", "experiment", "explore"}


@dataclass(slots=True)
class RecallResult:
    memory: MemoryEvent
    score: float
    ranking: dict[str, float]


class MemoryRetriever:
    def __init__(self, store: SQLiteMemoryStore) -> None:
        self.store = store

    def recall(
        self,
        query: str,
        *,
        project: str | None = None,
        agent_profile: str | None = None,
        scope: str | None = None,
        rings: list[str] | None = None,
        event_types: list[str] | None = None,
        include_sensitive: bool = False,
        include_superseded: bool = False,
        limit: int = 8,
        explain_ranking: bool = False,
    ) -> list[RecallResult]:
        candidates = self.store.search_text(query, include_superseded=include_superseded)
        filtered = [
            event for event in candidates
            if self._matches(event, project, agent_profile, scope, rings, event_types, include_sensitive)
        ]
        results = [self._score(event, query, explain_ranking) for event in filtered]
        results.sort(key=lambda item: item.score, reverse=True)
        return results[:limit]

    def _matches(
        self,
        event: MemoryEvent,
        project: str | None,
        agent_profile: str | None,
        scope: str | None,
        rings: list[str] | None,
        event_types: list[str] | None,
        include_sensitive: bool,
    ) -> bool:
        if project and event.project != project:
            return False
        if agent_profile and event.agent_profile != agent_profile:
            return False
        if scope and event.scope != scope:
            return False
        if rings and event.ring not in rings:
            return False
        if event_types and event.event_type not in event_types:
            return False
        if not include_sensitive and event.sensitivity != "normal":
            return False
        return True

    def _score(self, event: MemoryEvent, query: str, explain_ranking: bool) -> RecallResult:
        textual = self._textual_match(event, query)
        recency = self._recency_score(event.created_at)
        authority = self._source_authority(event)
        ring_boost = self._ring_boost(event, query)
        ranking = {
            "textual_match": textual,
            "salience": event.salience,
            "confidence": event.confidence,
            "recency": recency,
            "source_authority": authority,
            "ring_boost": ring_boost,
        }
        score = (
            0.25 * textual
            + 0.25 * event.salience
            + 0.25 * event.confidence
            + 0.20 * recency
            + 0.05 * authority
            + ring_boost
        )
        if not explain_ranking:
            ranking = {}
        return RecallResult(event, score, ranking)

    def _textual_match(self, event: MemoryEvent, query: str) -> float:
        terms = {term.lower() for term in query.split() if term.strip()}
        if not terms:
            return 0.1
        text = " ".join([event.summary, event.details, " ".join(event.tags)]).lower()
        matches = sum(1 for term in terms if term in text)
        return matches / len(terms)

    def _recency_score(self, created_at: datetime) -> float:
        age_days = max((datetime.now(UTC) - created_at).total_seconds() / 86400, 0)
        return exp(-age_days / 30)

    def _source_authority(self, event: MemoryEvent) -> float:
        order = {
            "user": 1.0,
            "contract": 0.9,
            "eval": 0.8,
            "file": 0.7,
            "tool": 0.6,
            "summary": 0.5,
            "manual": 0.4,
        }
        return order.get(event.source.type, 0.3)

    def _ring_boost(self, event: MemoryEvent, query: str) -> float:
        terms = {term.lower() for term in query.split()}
        if event.ring == "scar" and terms & FAILURE_TERMS:
            return 0.2
        if event.ring == "heartwood" and terms & HEARTWOOD_TERMS:
            return 0.15
        if event.ring == "seed" and terms & SEED_TERMS:
            return 0.12
        return 0.0
```

- [ ] **Step 4: Run recall tests**

Run:

```bash
pytest tests/test_recall.py -q
```

Expected:

```text
4 passed
```

- [ ] **Step 5: Commit**

```bash
git add src/tree_ring_memory/recall.py tests/test_recall.py
git commit -m "feat: add recall ranking"
```

---

### Task 7: Public Facade And Forget Flow

**Files:**
- Create: `src/tree_ring_memory/api.py`
- Modify: `src/tree_ring_memory/__init__.py`
- Add tests to: `tests/test_store.py`

- [ ] **Step 1: Add facade tests**

Append to `tests/test_store.py`:

```python
from tree_ring_memory import TreeRingMemory


def test_facade_remember_recall_and_forget(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")
    event = memory.remember(summary="Facade stores memory.", event_type="lesson", tags=["facade"])

    results = memory.recall("facade")
    assert results[0].memory.id == event.id

    memory.forget(event.id, mode="delete", reason="test cleanup")
    assert memory.recall("facade") == []


def test_facade_blocks_secret_by_default(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")

    try:
        memory.remember(summary="token = sk-proj-abcdefghijklmnopqrstuvwxyz1234567890", event_type="lesson")
    except ValueError as exc:
        assert "blocked" in str(exc)
    else:
        raise AssertionError("secret-like memory should be blocked")
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
pytest tests/test_store.py -q
```

Expected: import failure or missing `TreeRingMemory`.

- [ ] **Step 3: Implement facade**

Create `src/tree_ring_memory/api.py`:

```python
from __future__ import annotations

from pathlib import Path

from tree_ring_memory.models import MemoryEvent, MemorySource
from tree_ring_memory.recall import MemoryRetriever, RecallResult
from tree_ring_memory.sensitivity import SensitivityGuard
from tree_ring_memory.store import SQLiteMemoryStore


class TreeRingMemory:
    def __init__(self, root: Path, store: SQLiteMemoryStore, guard: SensitivityGuard) -> None:
        self.root = root
        self.store = store
        self.guard = guard
        self.retriever = MemoryRetriever(store)

    @classmethod
    def open(cls, root: str | Path) -> "TreeRingMemory":
        root = Path(root)
        root.mkdir(parents=True, exist_ok=True)
        store = SQLiteMemoryStore.open(root / "memory.sqlite")
        return cls(root, store, SensitivityGuard())

    def remember(
        self,
        *,
        summary: str,
        event_type: str,
        details: str = "",
        scope: str = "global",
        ring: str = "cambium",
        project: str | None = None,
        agent_profile: str | None = None,
        source: MemorySource | None = None,
        tags: list[str] | None = None,
        salience: float = 0.5,
        confidence: float = 0.5,
        retention: str = "normal",
    ) -> MemoryEvent:
        summary_check = self.guard.check_or_raise(summary)
        details_check = self.guard.check_or_raise(details) if details else None
        sensitivity = summary_check.sensitivity
        if details_check and details_check.sensitivity != "normal":
            sensitivity = details_check.sensitivity
        event = MemoryEvent.new(
            summary=summary_check.redacted_text,
            details=details_check.redacted_text if details_check else details,
            event_type=event_type,
            scope=scope,
            ring=ring,
            project=project,
            agent_profile=agent_profile,
            source=source or MemorySource(),
            tags=tags or [],
            salience=salience,
            confidence=confidence,
            sensitivity=sensitivity,
            retention=retention,
        )
        self.store.put(event)
        return event

    def recall(
        self,
        query: str,
        *,
        project: str | None = None,
        agent_profile: str | None = None,
        scope: str | None = None,
        rings: list[str] | None = None,
        event_types: list[str] | None = None,
        include_sensitive: bool = False,
        include_superseded: bool = False,
        limit: int = 8,
        explain_ranking: bool = False,
    ) -> list[RecallResult]:
        return self.retriever.recall(
            query,
            project=project,
            agent_profile=agent_profile,
            scope=scope,
            rings=rings,
            event_types=event_types,
            include_sensitive=include_sensitive,
            include_superseded=include_superseded,
            limit=limit,
            explain_ranking=explain_ranking,
        )

    def forget(self, memory_id: str, *, mode: str, reason: str) -> None:
        if not reason.strip():
            raise ValueError("reason is required")
        if mode == "delete":
            self.store.delete(memory_id)
            return
        if mode == "redact":
            event = self.store.get(memory_id)
            if event is None:
                return
            event.summary = "[REDACTED]"
            event.details = ""
            event.sensitivity = "private"
            self.store.put(event)
            return
        raise ValueError(f"unsupported forget mode: {mode}")
```

Confirm `src/tree_ring_memory/__init__.py` imports `TreeRingMemory` from `tree_ring_memory.api`.

- [ ] **Step 4: Run facade tests**

Run:

```bash
pytest tests/test_store.py -q
```

Expected:

```text
6 passed
```

- [ ] **Step 5: Commit**

```bash
git add src/tree_ring_memory/api.py src/tree_ring_memory/__init__.py tests/test_store.py
git commit -m "feat: add public memory facade"
```

---

### Task 8: CLI

**Files:**
- Create: `src/tree_ring_memory/cli.py`
- Create: `tests/test_cli.py`

- [ ] **Step 1: Write CLI tests**

Create `tests/test_cli.py`:

```python
import subprocess
import sys


def run_cli(*args, cwd):
    return subprocess.run(
        [sys.executable, "-m", "tree_ring_memory.cli", *args],
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )


def test_cli_init_creates_store(tmp_path):
    result = run_cli("init", cwd=tmp_path)

    assert result.returncode == 0
    assert (tmp_path / ".tree-ring" / "memory.sqlite").exists()
    assert "Tree Ring Memory initialized" in result.stdout


def test_cli_remember_and_recall(tmp_path):
    init = run_cli("init", cwd=tmp_path)
    assert init.returncode == 0

    remembered = run_cli("remember", "Use protocol-first design.", "--event-type", "decision", cwd=tmp_path)
    assert remembered.returncode == 0
    assert "mem_" in remembered.stdout

    recalled = run_cli("recall", "protocol", cwd=tmp_path)
    assert recalled.returncode == 0
    assert "Use protocol-first design." in recalled.stdout
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
pytest tests/test_cli.py -q
```

Expected: module failure for `tree_ring_memory.cli`.

- [ ] **Step 3: Implement CLI**

Create `src/tree_ring_memory/cli.py`:

```python
from __future__ import annotations

import argparse
from pathlib import Path
import sys

from tree_ring_memory import TreeRingMemory


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="tree-ring", description="Local tree-ring memory for AI agents.")
    parser.add_argument("--root", default=".tree-ring", help="Memory root directory.")
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("init", help="Initialize local memory storage.")

    remember = sub.add_parser("remember", help="Store a memory.")
    remember.add_argument("summary")
    remember.add_argument("--event-type", default="lesson")
    remember.add_argument("--ring", default="cambium")
    remember.add_argument("--scope", default="global")
    remember.add_argument("--project")
    remember.add_argument("--tag", action="append", default=[])

    recall = sub.add_parser("recall", help="Recall memory.")
    recall.add_argument("query")
    recall.add_argument("--project")
    recall.add_argument("--limit", type=int, default=8)
    recall.add_argument("--include-sensitive", action="store_true")

    forget = sub.add_parser("forget", help="Delete or redact a memory.")
    forget.add_argument("memory_id")
    forget.add_argument("--mode", choices=["delete", "redact"], default="delete")
    forget.add_argument("--reason", required=True)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    memory = TreeRingMemory.open(Path(args.root))

    if args.command == "init":
        print(f"Tree Ring Memory initialized at {Path(args.root)}")
        print("Secrets are blocked by default. No cloud service is required.")
        return 0

    if args.command == "remember":
        try:
            event = memory.remember(
                summary=args.summary,
                event_type=args.event_type,
                ring=args.ring,
                scope=args.scope,
                project=args.project,
                tags=args.tag,
            )
        except ValueError as exc:
            print(f"Could not store memory: {exc}", file=sys.stderr)
            return 2
        print(event.id)
        return 0

    if args.command == "recall":
        results = memory.recall(
            args.query,
            project=args.project,
            include_sensitive=args.include_sensitive,
            limit=args.limit,
        )
        for result in results:
            print(f"{result.memory.id} [{result.memory.ring}] {result.memory.summary} score={result.score:.3f}")
        return 0

    if args.command == "forget":
        memory.forget(args.memory_id, mode=args.mode, reason=args.reason)
        print(f"{args.mode} complete: {args.memory_id}")
        return 0

    parser.error("unknown command")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run CLI tests**

Run:

```bash
pytest tests/test_cli.py -q
```

Expected:

```text
2 passed
```

- [ ] **Step 5: Commit**

```bash
git add src/tree_ring_memory/cli.py tests/test_cli.py
git commit -m "feat: add tree-ring cli"
```

---

### Task 9: Full Test And Docs Pass

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README with CLI usage**

Add this section to `README.md` after the first Python example:

```markdown
## CLI Preview

```bash
tree-ring init
tree-ring remember "Use protocol-first design." --event-type decision --tag architecture
tree-ring recall "protocol design"
tree-ring forget mem_example --mode delete --reason "example cleanup"
```

The CLI stores memory in `.tree-ring/` by default.
```

- [ ] **Step 2: Run all tests**

Run:

```bash
pytest
```

Expected:

```text
19 passed
```

- [ ] **Step 3: Verify package import**

Run:

```bash
python -c "from tree_ring_memory import TreeRingMemory; print(TreeRingMemory)"
```

Expected output contains:

```text
<class 'tree_ring_memory.api.TreeRingMemory'>
```

- [ ] **Step 4: Verify CLI help**

Run:

```bash
python -m tree_ring_memory.cli --help
```

Expected output contains:

```text
Local tree-ring memory for AI agents.
```

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: document cli preview"
```

---

## Plan Self-Review

Spec coverage:

- Protocol docs and schemas: Task 3.
- Python reference implementation: Tasks 2, 5, 6, 7.
- Local SQLite storage: Task 5.
- Sensitivity and secret blocking: Task 4 and Task 7.
- remember, recall, forget: Tasks 7 and 8.
- CLI: Task 8 and Task 9.
- Testing: every implementation task starts with failing tests and ends with passing tests.

Deferred scope is explicit:

- consolidation, eval harness, import/export, adapters, sidecar, and workbench are staged after v0.1.

Placeholder check:

- The plan contains concrete files, commands, code blocks, and expected outputs.
- There are no unspecified implementation steps.

Type consistency:

- `MemoryEvent`, `MemorySource`, `SQLiteMemoryStore`, `MemoryRetriever`, `RecallResult`, and `TreeRingMemory` signatures are consistent across tasks.

Execution recommendation:

- Use subagent-driven development, one task per subagent, with review after each task.

