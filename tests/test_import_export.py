import json

from tree_ring_memory import NativeTreeRingMemory, PythonTreeRingMemory
from tree_ring_memory.models import MemoryEvent


def test_python_reference_jsonl_export_import_round_trip(tmp_path):
    source = PythonTreeRingMemory.open(tmp_path / "source")
    target = PythonTreeRingMemory.open(tmp_path / "target")
    event = source.remember(
        summary="Python JSONL round trip uses the shared schema.",
        event_type="lesson",
        project="compat",
    )

    jsonl = source.export_jsonl()
    dry_run_report = target.import_jsonl(jsonl, dry_run=True)
    import_report = target.import_jsonl(jsonl)

    header = json.loads(jsonl.splitlines()[0])
    assert header["type"] == "tree_ring_memory_export"
    assert header["schema_version"] == 1
    assert header["plugin_version"] == "0.7.0"
    assert event.id in jsonl
    assert dry_run_report == {
        "valid_count": 1,
        "inserted_count": 0,
        "replaced_count": 0,
        "skipped_duplicate_count": 0,
        "dry_run": True,
    }
    assert import_report["inserted_count"] == 1
    assert target.recall("shared schema", project="compat")[0].memory.id == event.id


def test_python_reference_jsonl_duplicate_and_replace_reports(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    event = memory.remember(summary="Original duplicate memory.", event_type="lesson")
    replacement = event.to_dict()
    replacement["summary"] = "Replacement duplicate memory."
    jsonl = json.dumps({"type": "memory_event", "memory": replacement}) + "\n"

    skipped = memory.import_jsonl(jsonl)
    replaced = memory.import_jsonl(jsonl, replace_existing=True)

    assert skipped["skipped_duplicate_count"] == 1
    assert replaced["replaced_count"] == 1
    assert memory.store.get(event.id).summary == "Replacement duplicate memory."


def test_python_reference_jsonl_export_filters_sensitive_and_superseded(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    old = memory.remember(summary="Old public decision.", event_type="decision")
    memory.remember(
        summary="New public decision.",
        event_type="decision",
        supersedes=[old.id],
    )
    sensitive = memory.remember(
        summary="Private import export context.",
        event_type="lesson",
        sensitivity="private",
    )

    default_jsonl = memory.export_jsonl()
    full_jsonl = memory.export_jsonl(include_sensitive=True, include_superseded=True)
    default_memory_ids = _memory_ids(default_jsonl)
    full_memory_ids = _memory_ids(full_jsonl)

    assert old.id not in default_memory_ids
    assert sensitive.id not in default_memory_ids
    assert old.id in full_memory_ids
    assert sensitive.id in full_memory_ids


def test_python_reference_import_reclassifies_sensitive_and_blocks_secrets(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    health = MemoryEvent.new(
        summary="Private diagnosis imported as normal.",
        event_type="lesson",
        sensitivity="normal",
    )
    health_jsonl = json.dumps({"type": "memory_event", "memory": health.to_dict()}) + "\n"

    report = memory.import_jsonl(health_jsonl)

    assert report["inserted_count"] == 1
    assert memory.store.get(health.id).sensitivity == "health"

    secret = MemoryEvent.new(
        summary="Imported secret sk-proj-abcdefghijklmnopqrstuvwxyz1234567890 must fail.",
        event_type="lesson",
    )
    secret_jsonl = json.dumps({"type": "memory_event", "memory": secret.to_dict()}) + "\n"

    try:
        memory.import_jsonl(secret_jsonl)
    except ValueError as exc:
        assert "blocked" in str(exc)
    else:
        raise AssertionError("secret import should have been blocked")


def test_python_reference_import_applies_supersedes_to_existing_target(tmp_path):
    source = PythonTreeRingMemory.open(tmp_path / "source")
    target = PythonTreeRingMemory.open(tmp_path / "target")
    old = target.remember(summary="Old imported decision.", event_type="decision")
    new = source.remember(
        summary="New imported decision.",
        event_type="decision",
        supersedes=[old.id],
    )
    jsonl = source.export_jsonl()

    report = target.import_jsonl(jsonl)

    assert report["inserted_count"] == 1
    assert target.store.get(old.id).superseded_by == new.id
    assert [event.id for event in target.store.list_all()] == [new.id]


def test_python_reference_import_applies_supersedes_after_all_rows_are_written(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    old = MemoryEvent.new(summary="Old decision imported after replacement.", event_type="decision")
    new = MemoryEvent.new(
        summary="New decision imported before old.",
        event_type="decision",
        supersedes=[old.id],
    )
    jsonl = "\n".join(
        [
            json.dumps(
                {
                    "type": "tree_ring_memory_export",
                    "schema_version": 1,
                    "plugin_version": "0.4.0",
                    "created_at": "2026-07-05T00:00:00+00:00",
                    "memory_count": 2,
                    "sensitive_included": False,
                }
            ),
            json.dumps({"type": "memory_event", "memory": new.to_dict()}),
            json.dumps({"type": "memory_event", "memory": old.to_dict()}),
        ]
    )

    report = memory.import_jsonl(jsonl)

    assert report["inserted_count"] == 2
    assert memory.store.get(old.id).superseded_by == new.id
    assert [event.id for event in memory.store.list_all()] == [new.id]


def test_python_reference_jsonl_rejects_malformed_export_header(tmp_path):
    memory = PythonTreeRingMemory.open(tmp_path / ".tree-ring")
    malformed = json.dumps({"type": "tree_ring_memory_export", "schema_version": 1}) + "\n"

    try:
        memory.import_jsonl(malformed, dry_run=True)
    except ValueError as exc:
        assert "missing export header fields" in str(exc)
        assert "plugin_version" in str(exc)
    else:
        raise AssertionError("malformed export header should have been rejected")


def test_native_backend_jsonl_methods_delegate_to_binding(tmp_path):
    class FakeNativeStore:
        def __init__(self):
            self.export_kwargs = None
            self.import_kwargs = None

        def export_jsonl(self, *, include_sensitive=False, include_superseded=False):
            self.export_kwargs = {
                "include_sensitive": include_sensitive,
                "include_superseded": include_superseded,
            }
            return '{"type":"tree_ring_memory_export"}\n'

        def import_jsonl(self, data, *, dry_run=False, replace_existing=False):
            self.import_kwargs = {
                "data": data,
                "dry_run": dry_run,
                "replace_existing": replace_existing,
            }
            return json.dumps(
                {
                    "valid_count": 1,
                    "inserted_count": 0,
                    "replaced_count": 0,
                    "skipped_duplicate_count": 0,
                    "dry_run": dry_run,
                }
            )

    fake_native = FakeNativeStore()
    memory = NativeTreeRingMemory(fake_native, tmp_path / ".tree-ring")

    jsonl = memory.export_jsonl(include_sensitive=True, include_superseded=True)
    report = memory.import_jsonl(jsonl, dry_run=True, replace_existing=True)

    assert fake_native.export_kwargs == {
        "include_sensitive": True,
        "include_superseded": True,
    }
    assert fake_native.import_kwargs == {
        "data": jsonl,
        "dry_run": True,
        "replace_existing": True,
    }
    assert report["valid_count"] == 1
    assert report["dry_run"] is True


def _memory_ids(jsonl: str) -> set[str]:
    ids = set()
    for line in jsonl.splitlines():
        record = json.loads(line)
        if record.get("type") == "memory_event":
            ids.add(record["memory"]["id"])
    return ids
