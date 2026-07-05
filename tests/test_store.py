import json

import pytest

from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource
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


def test_store_enables_wal_and_busy_timeout(tmp_path):
    store = SQLiteMemoryStore.open(tmp_path / "memory.sqlite")

    journal_mode = store.connection.execute("PRAGMA journal_mode").fetchone()[0]
    busy_timeout = store.connection.execute("PRAGMA busy_timeout").fetchone()[0]

    assert journal_mode.lower() == "wal"
    assert busy_timeout >= 30000


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


def test_facade_blocks_secret_tag_by_default(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")

    with pytest.raises(ValueError, match="blocked"):
        memory.remember(
            summary="Facade should guard indexed tags.",
            event_type="lesson",
            tags=["sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"],
        )


def test_facade_blocks_secret_source_ref_by_default(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")

    with pytest.raises(ValueError, match="blocked"):
        memory.remember(
            summary="Facade should guard indexed source refs.",
            event_type="lesson",
            source=MemorySource(type="manual", ref="sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"),
        )


def test_facade_blocks_secret_link_target_by_default(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")

    with pytest.raises(ValueError, match="blocked"):
        memory.remember(
            summary="Facade should guard link targets.",
            event_type="lesson",
            links=[MemoryLink(type="url", target="sk-proj-abcdefghijklmnopqrstuvwxyz1234567890")],
        )


def test_facade_blocks_secret_supersedes_by_default(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")

    with pytest.raises(ValueError, match="blocked"):
        memory.remember(
            summary="Facade should guard supersession ids.",
            event_type="lesson",
            supersedes=["sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"],
        )


@pytest.mark.parametrize(
    "review",
    [
        MemoryReview(review_reason="sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"),
        MemoryReview(reviewed_at="sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"),
        MemoryReview(reviewed_by="sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"),
    ],
)
def test_facade_blocks_secret_review_fields_by_default(tmp_path, review):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")

    with pytest.raises(ValueError, match="blocked"):
        memory.remember(
            summary="Facade should guard review metadata.",
            event_type="lesson",
            review=review,
        )


def test_facade_supersedes_hides_old_memory_by_default(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")
    old = memory.remember(summary="Use polling invalidation.", event_type="decision")
    new = memory.remember(
        summary="Use snapshot invalidation.",
        event_type="decision",
        supersedes=[old.id],
    )

    assert memory.store.get(old.id).superseded_by == new.id
    assert memory.recall("polling") == []
    assert [result.memory.id for result in memory.recall("polling", include_superseded=True)] == [old.id]


def test_facade_redact_clears_secret_source_ref_from_storage_and_recall(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")
    secret_ref = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"
    event = MemoryEvent.new(
        summary="Legacy memory with source ref.",
        event_type="lesson",
        details="details should be cleared",
        source=MemorySource(type="manual", ref=secret_ref, quote="quoted secret context"),
        tags=["secret-tag"],
        sensitivity="secret",
    )
    memory.store.put(event)

    memory.forget(event.id, mode="redact", reason="remove legacy secret source ref")

    redacted = memory.store.get(event.id)
    assert redacted is not None
    assert redacted.summary == "[REDACTED]"
    assert redacted.details == ""
    assert redacted.source.ref == ""
    assert redacted.source.quote == ""
    assert redacted.tags == []
    assert redacted.sensitivity == "private"
    assert secret_ref not in str(redacted.to_dict())
    assert memory.recall(secret_ref, include_sensitive=True) == []
    assert memory.store.search_text(secret_ref, include_superseded=True) == []


def test_facade_redact_clears_secret_metadata_from_storage_and_recall(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")
    secret = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"
    event = MemoryEvent.new(
        summary="Legacy memory with secret metadata.",
        event_type=secret,
        project=secret,
        agent_profile=secret,
        details="details should be cleared",
        source=MemorySource(type=secret, ref=secret, quote=secret),
        tags=[secret],
        sensitivity="secret",
        supersedes=[secret],
        links=[MemoryLink(type="url", target=secret)],
        review=MemoryReview(
            needs_review=True,
            review_reason=secret,
            reviewed_at=secret,
            reviewed_by=secret,
        ),
    )
    memory.store.put(event)

    memory.forget(event.id, mode="redact", reason="remove legacy secret metadata")

    redacted = memory.store.get(event.id)
    assert redacted is not None
    assert redacted.summary == "[REDACTED]"
    assert redacted.details == ""
    assert redacted.project is None
    assert redacted.agent_profile is None
    assert redacted.event_type == "redacted"
    assert redacted.source == MemorySource()
    assert redacted.tags == []
    assert redacted.supersedes == []
    assert redacted.links == []
    assert redacted.review == MemoryReview()
    assert redacted.sensitivity == "private"
    assert secret not in json.dumps(redacted.to_dict(), sort_keys=True)
    assert memory.recall(secret, include_sensitive=True) == []
    assert memory.store.search_text(secret, include_superseded=True) == []


def test_facade_redact_clears_secret_superseded_by_from_storage(tmp_path):
    memory = TreeRingMemory.open(tmp_path / ".tree-ring")
    secret = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"
    old = MemoryEvent.new(summary="Legacy superseded memory.", event_type="lesson")
    memory.store.put(old)
    memory.store.supersede(old.id, secret)

    legacy = memory.store.get(old.id)
    assert legacy is not None
    assert legacy.superseded_by == secret

    memory.forget(old.id, mode="redact", reason="remove legacy secret superseded metadata")

    redacted = memory.store.get(old.id)
    assert redacted is not None
    assert redacted.superseded_by is None
    assert secret not in json.dumps(redacted.to_dict(), sort_keys=True)

    row = memory.store.connection.execute(
        "SELECT superseded_by, raw_json FROM memories WHERE id = ?",
        (old.id,),
    ).fetchone()
    assert row["superseded_by"] is None
    assert secret not in row["raw_json"]
