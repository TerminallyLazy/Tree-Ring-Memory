import pytest

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


@pytest.mark.parametrize(
    "query",
    [
        "C++",
        "cache?",
        '"unterminated',
        "cache -missing",
        "what about cache invalidation?",
        "source_ref:test",
        "cache OR SQLite",
    ],
)
def test_search_text_treats_user_query_as_plain_text(tmp_path, query):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="Avoid stale cache without invalidation.", event_type="warning", ring="scar"))
    store.put(
        MemoryEvent.new(
            summary="Use local SQLite for v0.1.",
            event_type="decision",
            ring="heartwood",
            source=MemorySource(type="manual", ref="test"),
        )
    )

    results = store.search_text(query)

    assert isinstance(results, list)


@pytest.mark.parametrize("query", ["cache?", "what about cache invalidation?"])
def test_search_text_finds_relevant_memory_from_natural_punctuation(tmp_path, query):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    event = MemoryEvent.new(summary="Avoid stale cache invalidation bugs.", event_type="warning", ring="scar")
    store.put(event)

    results = store.search_text(query)

    assert [result.id for result in results] == [event.id]


def test_search_text_does_not_treat_column_filter_as_fts_syntax(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(
        MemoryEvent.new(
            summary="Use local SQLite for v0.1.",
            event_type="decision",
            source=MemorySource(type="manual", ref="test"),
        )
    )

    assert store.search_text("source_ref:test") == []


def test_search_text_does_not_treat_or_as_boolean_operator(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")
    store.put(MemoryEvent.new(summary="Avoid stale cache bugs.", event_type="warning", ring="scar"))
    store.put(MemoryEvent.new(summary="Use local SQLite for v0.1.", event_type="decision", ring="heartwood"))

    assert store.search_text("cache OR SQLite") == []
