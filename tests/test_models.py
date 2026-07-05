import re
from datetime import UTC, datetime
from math import nan

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


def test_generated_memory_id_uses_random_hex_suffix():
    event = MemoryEvent.new(summary="Use collision-resistant ids.", event_type="decision")
    suffix = event.id.rsplit("_", 1)[1]

    assert re.fullmatch(r"[0-9a-f]{12}", suffix)
    assert suffix != "000001"


def test_missing_summary_is_rejected():
    with pytest.raises(ValidationError, match="summary is required"):
        MemoryEvent.new(summary="", event_type="decision")


def test_invalid_ring_is_rejected():
    with pytest.raises(ValidationError, match="invalid ring"):
        MemoryEvent.new(summary="Bad ring.", event_type="decision", ring="bark")


def test_invalid_score_is_rejected():
    with pytest.raises(ValidationError, match="salience"):
        MemoryEvent.new(summary="Bad score.", event_type="decision", salience=2.0)


def test_invalid_confidence_score_is_rejected():
    with pytest.raises(ValidationError, match="confidence"):
        MemoryEvent.new(summary="Bad score.", event_type="decision", confidence=2.0)


@pytest.mark.parametrize(
    ("score_name", "score_value"),
    [
        ("salience", None),
        ("confidence", "not-a-number"),
    ],
)
def test_invalid_score_type_is_validation_error(score_name, score_value):
    with pytest.raises(ValidationError, match=score_name):
        MemoryEvent.new(summary="Bad score.", event_type="decision", **{score_name: score_value})


@pytest.mark.parametrize("score_name", ["salience", "confidence"])
def test_nan_score_is_rejected(score_name):
    with pytest.raises(ValidationError, match=score_name):
        MemoryEvent.new(summary="Bad score.", event_type="decision", **{score_name: nan})


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
