from datetime import datetime

import pytest

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


@pytest.mark.parametrize("query", ["source_ref:test", "cache OR SQLite"])
def test_recall_does_not_broaden_empty_fts_results(tmp_path, query):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(
        MemoryEvent.new(
            summary="Use local SQLite for v0.1.",
            event_type="decision",
            source=MemorySource(type="manual", ref="test"),
        )
    )
    store.put(MemoryEvent.new(summary="Avoid stale cache bugs.", event_type="warning", ring="scar"))

    results = MemoryRetriever(store).recall(query)

    assert results == []


@pytest.mark.parametrize("query", ["", " \t\n "])
def test_recall_empty_query_returns_no_results(tmp_path, query):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="Normal memory", event_type="lesson"))

    results = MemoryRetriever(store).recall(query)

    assert results == []


def test_recall_scores_naive_created_at_as_utc(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    event = MemoryEvent.new(summary="Naive memory timestamp.", event_type="lesson")
    payload = event.to_dict()
    payload["created_at"] = "2026-01-01T00:00:00"
    payload["updated_at"] = "2026-01-01T00:00:00"
    store.put(MemoryEvent.from_dict(payload))

    results = MemoryRetriever(store).recall("naive memory", explain_ranking=True)

    assert results[0].memory.created_at == datetime(2026, 1, 1, 0, 0, 0)
    assert results[0].ranking["recency"] >= 0
