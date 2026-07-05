import json
from datetime import datetime, timedelta

import pytest

from tree_ring_memory import PythonTreeRingMemory
from tree_ring_memory.models import MemoryEvent, now_utc
from tree_ring_memory.native_backend import NativeTreeRingMemory
from tree_ring_memory.store import SQLiteMemoryStore


def finding_types(report):
    return {finding["audit_type"] for finding in report["findings"]}


def test_python_reference_audit_reports_all_deterministic_checks(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    expired = MemoryEvent.new(
        summary="Expired advisory retention.",
        event_type="lesson",
        expires_at=now_utc() - timedelta(days=1),
    )
    sensitive = MemoryEvent.new(
        summary="Sensitive policy row.",
        event_type="lesson",
        sensitivity="health",
        retention="durable",
    )
    low_confidence = MemoryEvent.new(
        summary="Durable low-confidence row.",
        event_type="decision",
        ring="heartwood",
        confidence=0.4,
        retention="durable",
    )
    superseding = MemoryEvent.new(
        summary="New decision references missing old memory.",
        event_type="decision",
        supersedes=["mem_missing_old"],
    )
    use = MemoryEvent.new(
        summary="Use cache invalidation.",
        event_type="decision",
        scope="project",
        project="audit",
        tags=["cache"],
    )
    avoid = MemoryEvent.new(
        summary="Avoid cache invalidation.",
        event_type="decision",
        scope="project",
        project="audit",
        tags=["cache"],
    )
    for event in [expired, sensitive, low_confidence, superseding, use, avoid]:
        store.put(event)

    report = store.audit()

    assert report["audit_type"] == "all"
    assert report["memory_count"] == 6
    assert finding_types(report) == {
        "stale",
        "sensitive",
        "low_confidence",
        "supersession",
        "contradictions",
    }


def test_python_reference_audit_filters_by_type_and_rejects_unknown_type(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(
        MemoryEvent.new(
            summary="Expired advisory retention.",
            event_type="lesson",
            expires_at=now_utc() - timedelta(days=1),
        )
    )

    report = store.audit("stale")

    assert report["audit_type"] == "stale"
    assert finding_types(report) == {"stale"}
    with pytest.raises(ValueError, match="unsupported audit_type"):
        store.audit("unknown")


def test_python_reference_audit_matches_rust_payload_contract(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    naive = MemoryEvent.new(
        summary="Naive expiry should be treated as invalid.",
        event_type="lesson",
        expires_at=datetime(2026, 1, 1, 0, 0, 0),
    )
    sensitive = MemoryEvent.new(
        summary="Sensitive durable row.",
        event_type="lesson",
        sensitivity="secret",
        retention="durable",
    )
    low_confidence = MemoryEvent.new(
        summary="Weak heartwood row.",
        event_type="decision",
        ring="heartwood",
        confidence=0.4,
        retention="durable",
    )
    superseding = MemoryEvent.new(
        summary="Replacement decision references missing old memory.",
        event_type="decision",
        supersedes=["mem_missing_old"],
    )
    use = MemoryEvent.new(
        summary="Use cache invalidation.",
        event_type="decision",
        scope="project",
        project="audit",
        tags=["cache"],
    )
    avoid = MemoryEvent.new(
        summary="Avoid cache invalidation.",
        event_type="decision",
        scope="project",
        project="audit",
        tags=["cache"],
    )
    for event in [naive, sensitive, low_confidence, superseding, use, avoid]:
        store.put(event)

    report = store.audit()

    findings = {(finding["audit_type"], finding["severity"], finding["finding"]): finding for finding in report["findings"]}
    assert findings[("stale", "medium", "Memory has an invalid expires_at timestamp.")][
        "recommended_action"
    ] == "Review the memory and set a valid ISO-8601 expires_at value or redact it."
    assert findings[("sensitive", "critical", "Secret-like memory is retained.")][
        "recommended_action"
    ] == "Redact or delete this memory."
    assert findings[("sensitive", "high", "Sensitive memory has durable retention.")][
        "tags"
    ] == ["privacy", "retention"]
    assert findings[("sensitive", "medium", "Sensitive memory is retained without an expiry.")][
        "tags"
    ] == ["privacy", "expiry"]
    assert findings[("low_confidence", "high", "Heartwood memory has low confidence.")][
        "tags"
    ] == ["confidence", "heartwood"]
    assert findings[("low_confidence", "medium", "Durable memory has very low confidence.")][
        "recommended_action"
    ] == "Review, demote, or supersede this memory."
    assert findings[("supersession", "medium", "Memory supersedes a missing memory.")][
        "recommended_action"
    ] == "Import, restore, or remove the missing supersedes reference."
    contradiction = findings[
        ("contradictions", "medium", "Memories contain contradictory use/avoid guidance.")
    ]
    assert contradiction["memory_id"] == use.id
    assert contradiction["related_memory_id"] == avoid.id
    assert contradiction["tags"] == ["contradiction", "review"]


def test_python_reference_audit_detects_supersession_reciprocal_gap(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    old = MemoryEvent.new(summary="Old decision.", event_type="decision")
    new = MemoryEvent.new(summary="New decision.", event_type="decision", supersedes=[old.id])
    store.put(old)
    store.put(new)

    report = store.audit("supersession")

    assert report["finding_count"] == 1
    finding = report["findings"][0]
    assert finding["memory_id"] == new.id
    assert finding["related_memory_id"] == old.id
    assert "reciprocal" in finding["finding"]


def test_python_reference_audit_is_non_mutating_and_does_not_leak_payloads(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    secret_payload = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"
    event = MemoryEvent.new(
        summary=f"Sensitive summary contains {secret_payload}.",
        event_type="lesson",
        sensitivity="secret",
    )
    store.put(event)
    before = store.connection.execute("SELECT raw_json FROM memories WHERE id = ?", (event.id,)).fetchone()[0]

    report = store.audit("sensitive")

    after = store.connection.execute("SELECT raw_json FROM memories WHERE id = ?", (event.id,)).fetchone()[0]
    assert before == after
    encoded_report = json.dumps(report)
    assert event.id in encoded_report
    assert secret_payload not in encoded_report
    assert event.summary not in encoded_report


def test_python_facade_delegates_audit_to_reference_store(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    event = memory.remember(
        summary="Heartwood low confidence should be audited.",
        event_type="lesson",
        ring="heartwood",
        confidence=0.2,
    )

    report = memory.audit("low_confidence")

    assert report["finding_count"] == 1
    assert report["findings"][0]["memory_id"] == event.id


def test_native_facade_parses_audit_json():
    class FakeNative:
        def audit_json(self, audit_type):
            assert audit_type == "sensitive"
            return json.dumps(
                {
                    "generated_at": "2026-07-05T00:00:00Z",
                    "audit_type": "sensitive",
                    "memory_count": 0,
                    "finding_count": 0,
                    "findings": [],
                }
            )

    memory = NativeTreeRingMemory(FakeNative(), root=".")

    assert memory.audit("sensitive")["audit_type"] == "sensitive"
