import json
import os
import subprocess
import sys
import types
from pathlib import Path

import pytest

from tree_ring_memory import TreeRingMemory
import tree_ring_memory.native_backend as native_backend
from tree_ring_memory.native_backend import NativeTreeRingMemory
from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource
from tree_ring_memory.store import SQLiteMemoryStore
from tree_ring_memory.rust_backend import RustCliTreeRingMemory


PROJECT_SRC = Path(__file__).resolve().parents[1] / "src"
PROJECT_ROOT = Path(__file__).resolve().parents[1]


def run_cli(*args, cwd):
    env = os.environ.copy()
    pythonpath = env.get("PYTHONPATH")
    env["PYTHONPATH"] = str(PROJECT_SRC) if not pythonpath else f"{PROJECT_SRC}{os.pathsep}{pythonpath}"
    return subprocess.run(
        [sys.executable, "-m", "tree_ring_memory.cli", *args],
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def run_rust_cli(root, *args):
    return subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "tree-ring-memory-cli",
            "--",
            "--root",
            str(root),
            *args,
        ],
        cwd=PROJECT_ROOT,
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


def test_cli_remember_secret_tag_returns_policy_error(tmp_path):
    secret_token = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"

    remembered = run_cli(
        "remember",
        "Facade should guard indexed tags.",
        "--event-type",
        "lesson",
        "--tag",
        secret_token,
        cwd=tmp_path,
    )

    assert remembered.returncode == 2
    assert "blocked" in remembered.stderr
    assert "Traceback" not in remembered.stderr


def test_cli_forget_blank_reason_returns_controlled_error(tmp_path):
    forgotten = run_cli("forget", "mem_missing", "--reason", "   ", cwd=tmp_path)

    assert forgotten.returncode == 2
    assert "forget reason is required" in forgotten.stderr
    assert "Traceback" not in forgotten.stderr


def test_rust_cli_written_memory_is_python_readable(tmp_path):
    cargo = run_rust_cli(
        tmp_path / ".tree-ring",
        "remember",
        "Rust writes schema-compatible memory.",
        "--event-type",
        "lesson",
        "--project",
        "compat",
    )
    assert cargo.returncode == 0, cargo.stderr
    memory_id = cargo.stdout.strip()

    store = SQLiteMemoryStore.open(tmp_path / ".tree-ring" / "memory.sqlite")
    event = store.get(memory_id)

    assert event is not None
    assert event.summary == "Rust writes schema-compatible memory."
    assert event.project == "compat"


def test_rust_cli_json_contract_for_init_remember_recall_and_forget(tmp_path):
    root = tmp_path / ".tree-ring"

    init = run_rust_cli(root, "--json", "init")
    assert init.returncode == 0, init.stderr
    init_payload = json.loads(init.stdout)
    assert init_payload["ok"] is True
    assert init_payload["message"] == "Tree Ring Memory initialized"

    remembered = run_rust_cli(
        root,
        "--json",
        "remember",
        "Use JSON bridge contract.",
        "--event-type",
        "lesson",
        "--project",
        "compat",
        "--tag",
        "json",
    )
    assert remembered.returncode == 0, remembered.stderr
    memory_payload = json.loads(remembered.stdout)
    assert memory_payload["id"].startswith("mem_")
    assert memory_payload["source"]["ref"] == ""
    assert "ref_" not in memory_payload["source"]

    recalled = run_rust_cli(root, "--json", "recall", "JSON bridge", "--project", "compat")
    assert recalled.returncode == 0, recalled.stderr
    recall_payload = json.loads(recalled.stdout)
    assert recall_payload[0]["memory"]["id"] == memory_payload["id"]
    assert recall_payload[0]["score"] > 0

    forgotten = run_rust_cli(
        root,
        "--json",
        "forget",
        memory_payload["id"],
        "--mode",
        "delete",
        "--reason",
        "test cleanup",
    )
    assert forgotten.returncode == 0, forgotten.stderr
    forget_payload = json.loads(forgotten.stdout)
    assert forget_payload == {"ok": True, "memory_id": memory_payload["id"]}


def test_rust_cli_export_import_jsonl_round_trip(tmp_path):
    source_root = tmp_path / "source" / ".tree-ring"
    target_root = tmp_path / "target" / ".tree-ring"
    export_path = tmp_path / "memories.jsonl"

    remembered = run_rust_cli(
        source_root,
        "--json",
        "remember",
        "Exported Rust memory survives import.",
        "--event-type",
        "lesson",
        "--project",
        "compat",
        "--tag",
        "export",
    )
    assert remembered.returncode == 0, remembered.stderr
    memory_payload = json.loads(remembered.stdout)

    exported = run_rust_cli(source_root, "--json", "export", "--output", str(export_path))
    assert exported.returncode == 0, exported.stderr
    export_report = json.loads(exported.stdout)
    assert export_report["memory_count"] == 1
    assert export_path.exists()
    exported_lines = [json.loads(line) for line in export_path.read_text().splitlines()]
    assert exported_lines[0]["type"] == "tree_ring_memory_export"
    assert exported_lines[0]["schema_version"] == 1
    assert exported_lines[0]["plugin_version"] == "0.5.0"
    assert exported_lines[1]["type"] == "memory_event"

    preview = run_rust_cli(target_root, "--json", "import", str(export_path), "--dry-run")
    assert preview.returncode == 0, preview.stderr
    preview_report = json.loads(preview.stdout)
    assert preview_report["valid_count"] == 1
    assert preview_report["inserted_count"] == 0
    assert not target_root.exists()

    imported = run_rust_cli(target_root, "--json", "import", str(export_path))
    assert imported.returncode == 0, imported.stderr
    import_report = json.loads(imported.stdout)
    assert import_report["inserted_count"] == 1
    assert import_report["skipped_duplicate_count"] == 0

    recalled = run_rust_cli(target_root, "--json", "recall", "survives import", "--project", "compat")
    assert recalled.returncode == 0, recalled.stderr
    recall_payload = json.loads(recalled.stdout)
    assert recall_payload[0]["memory"]["id"] == memory_payload["id"]


def test_rust_cli_export_excludes_sensitive_by_default(tmp_path):
    root = tmp_path / ".tree-ring"
    normal = run_rust_cli(
        root,
        "--json",
        "remember",
        "Normal export memory.",
        "--event-type",
        "lesson",
    )
    assert normal.returncode == 0, normal.stderr
    sensitive = run_rust_cli(
        root,
        "--json",
        "remember",
        "Private diagnosis should require explicit export.",
        "--event-type",
        "lesson",
    )
    assert sensitive.returncode == 0, sensitive.stderr

    default_export = run_rust_cli(root, "export")
    assert default_export.returncode == 0, default_export.stderr
    assert "Normal export memory." in default_export.stdout
    assert "Private diagnosis" not in default_export.stdout

    sensitive_export = run_rust_cli(root, "export", "--include-sensitive")
    assert sensitive_export.returncode == 0, sensitive_export.stderr
    assert "Private diagnosis" in sensitive_export.stdout


def test_rust_cli_import_skips_duplicates_by_default(tmp_path):
    source_root = tmp_path / "source" / ".tree-ring"
    target_root = tmp_path / "target" / ".tree-ring"
    export_path = tmp_path / "memories.jsonl"

    remembered = run_rust_cli(
        source_root,
        "--json",
        "remember",
        "Duplicate import should skip existing id.",
        "--event-type",
        "lesson",
    )
    assert remembered.returncode == 0, remembered.stderr
    run_rust_cli(source_root, "export", "--output", str(export_path))

    first = run_rust_cli(target_root, "--json", "import", str(export_path))
    second = run_rust_cli(target_root, "--json", "import", str(export_path))

    assert first.returncode == 0, first.stderr
    assert second.returncode == 0, second.stderr
    first_report = json.loads(first.stdout)
    second_report = json.loads(second.stdout)
    assert first_report["inserted_count"] == 1
    assert second_report["inserted_count"] == 0
    assert second_report["skipped_duplicate_count"] == 1


def test_rust_cli_import_dry_run_blocks_secret_without_creating_root(tmp_path):
    target_root = tmp_path / "target" / ".tree-ring"
    export_path = tmp_path / "secret.jsonl"
    secret = MemoryEvent.new(
        summary="Imported secret sk-proj-abcdefghijklmnopqrstuvwxyz1234567890 must fail.",
        event_type="lesson",
    )
    export_path.write_text(
        "\n".join(
            [
                json.dumps(
                    {
                        "type": "tree_ring_memory_export",
                        "schema_version": 1,
                        "plugin_version": "0.4.0",
                        "created_at": "2026-07-05T00:00:00+00:00",
                        "memory_count": 1,
                        "sensitive_included": True,
                    }
                ),
                json.dumps({"type": "memory_event", "memory": secret.to_dict()}),
            ]
        )
        + "\n"
    )

    preview = run_rust_cli(target_root, "--json", "import", str(export_path), "--dry-run")

    assert preview.returncode == 2
    assert "blocked" in preview.stderr
    assert not target_root.exists()


def test_rust_cli_audit_json_reports_sensitive_memory(tmp_path):
    root = tmp_path / ".tree-ring"
    remembered = run_rust_cli(
        root,
        "--json",
        "remember",
        "Private diagnosis should be reviewed by audit.",
        "--event-type",
        "lesson",
    )
    assert remembered.returncode == 0, remembered.stderr

    audited = run_rust_cli(root, "--json", "audit", "--audit-type", "sensitive")

    assert audited.returncode == 0, audited.stderr
    report = json.loads(audited.stdout)
    assert report["audit_type"] == "sensitive"
    assert report["memory_count"] == 1
    assert report["finding_count"] >= 1
    assert report["findings"][0]["audit_type"] == "sensitive"


def test_python_written_rich_memory_is_rust_recall_json_readable(tmp_path):
    root = tmp_path / ".tree-ring"
    memory = TreeRingMemory.open(root)
    event = memory.remember(
        summary="Python writes rich memory for Rust recall.",
        details="Rust should preserve source refs and details.",
        event_type="lesson",
        project="compat",
        source=MemorySource(type="file", ref="README.md", quote=""),
        tags=["python", "rust"],
    )

    recalled = run_rust_cli(root, "--json", "recall", "rich memory", "--project", "compat")
    assert recalled.returncode == 0, recalled.stderr
    payload = json.loads(recalled.stdout)

    assert payload[0]["memory"]["id"] == event.id
    assert payload[0]["memory"]["details"] == "Rust should preserve source refs and details."
    assert payload[0]["memory"]["source"]["ref"] == "README.md"


def test_rust_cli_backend_preserves_python_facade_shapes(tmp_path):
    memory = RustCliTreeRingMemory.open(tmp_path / ".tree-ring")

    event = memory.remember(
        summary="Rust backend preserves Python shapes.",
        event_type="lesson",
        project="compat",
        tags=["rust"],
    )
    results = memory.recall("Python shapes", project="compat")

    assert event.summary == "Rust backend preserves Python shapes."
    assert results[0].memory.id == event.id
    assert results[0].memory.tags == ["rust"]

    memory.forget(event.id, mode="delete", reason="test cleanup")
    assert memory.recall("Python shapes", project="compat") == []


def test_rust_cli_backend_rejects_unsupported_python_facade_fields(tmp_path):
    memory = RustCliTreeRingMemory.open(tmp_path / ".tree-ring")

    with pytest.raises(NotImplementedError, match="details"):
        memory.remember(
            summary="Unsupported details should fail explicitly.",
            details="not yet supported by the Rust CLI bridge",
            event_type="lesson",
        )


def test_native_backend_reports_missing_extension_cleanly(tmp_path):
    with pytest.raises(ImportError, match="native bindings are not installed"):
        NativeTreeRingMemory.open(tmp_path / ".tree-ring")


def test_native_backend_preserves_broken_extension_import_error(tmp_path, monkeypatch):
    def broken_import(name):
        assert name == "tree_ring_memory._tree_ring_memory_native"
        raise ImportError("dlopen failed")

    monkeypatch.setattr(native_backend.importlib, "import_module", broken_import)

    with pytest.raises(ImportError, match="dlopen failed"):
        NativeTreeRingMemory.open(tmp_path / ".tree-ring")


def test_default_facade_does_not_fallback_when_native_extension_is_broken(tmp_path, monkeypatch):
    def broken_import(name):
        assert name == "tree_ring_memory._tree_ring_memory_native"
        raise ImportError("dlopen failed")

    monkeypatch.setattr(native_backend.importlib, "import_module", broken_import)

    with pytest.raises(ImportError, match="dlopen failed"):
        TreeRingMemory.open(tmp_path / ".tree-ring")


def test_default_facade_uses_native_backend_when_extension_is_available(tmp_path, monkeypatch):
    class FakeNativeStore:
        @staticmethod
        def open(root):
            return {"root": root}

    fake_module = types.SimpleNamespace(TreeRingMemoryNative=FakeNativeStore)
    monkeypatch.setattr(
        native_backend.importlib,
        "import_module",
        lambda name: fake_module if name == "tree_ring_memory._tree_ring_memory_native" else None,
    )

    memory = TreeRingMemory.open(tmp_path / ".tree-ring")

    assert isinstance(memory, NativeTreeRingMemory)
    assert memory.backend_name == "rust-native"


def test_native_backend_remember_sends_full_facade_contract(tmp_path):
    class FakeNativeStore:
        def __init__(self):
            self.request = None

        def remember_event_json(self, request_json):
            self.request = json.loads(request_json)
            return json.dumps(
                {
                    "id": "mem_native",
                    "created_at": "2026-07-05T00:00:00+00:00",
                    "updated_at": "2026-07-05T00:00:00+00:00",
                    "project": self.request["project"],
                    "agent_profile": self.request["agent_profile"],
                    "scope": self.request["scope"],
                    "ring": self.request["ring"],
                    "event_type": self.request["event_type"],
                    "summary": self.request["summary"],
                    "details": self.request["details"],
                    "source": self.request["source"],
                    "tags": self.request["tags"],
                    "salience": self.request["salience"],
                    "confidence": self.request["confidence"],
                    "sensitivity": self.request["sensitivity"],
                    "retention": self.request["retention"],
                    "expires_at": self.request["expires_at"],
                    "supersedes": self.request["supersedes"],
                    "superseded_by": None,
                    "links": self.request["links"],
                    "review": self.request["review"],
                }
            )

    fake_native = FakeNativeStore()
    memory = NativeTreeRingMemory(fake_native, tmp_path / ".tree-ring")

    event = memory.remember(
        summary="Rust owns wrapper contract.",
        details="Full detail",
        event_type="decision",
        scope="project",
        ring="heartwood",
        project="migration",
        agent_profile="default",
        source=MemorySource(type="file", ref="README.md"),
        tags=["rust"],
        salience=0.7,
        confidence=0.8,
        sensitivity="private",
        retention="durable",
        supersedes=["mem_old"],
        links=[MemoryLink(type="file", target="README.md")],
        review=MemoryReview(needs_review=True, review_reason="parity"),
    )

    assert event.id == "mem_native"
    assert fake_native.request["details"] == "Full detail"
    assert fake_native.request["source"]["ref"] == "README.md"
    assert fake_native.request["links"] == [{"type": "file", "target": "README.md"}]
    assert fake_native.request["review"]["needs_review"] is True


def test_native_backend_recall_sends_full_filter_contract(tmp_path):
    class FakeNativeStore:
        def __init__(self):
            self.request = None

        def recall_query_json(self, request_json):
            self.request = json.loads(request_json)
            return "[]"

    fake_native = FakeNativeStore()
    memory = NativeTreeRingMemory(fake_native, tmp_path / ".tree-ring")

    assert memory.recall(
        "migration",
        project="tree-ring",
        agent_profile="default",
        scope="project",
        rings=["heartwood"],
        event_types=["decision"],
        include_sensitive=True,
        include_superseded=True,
        limit=3,
        explain_ranking=True,
    ) == []
    assert fake_native.request == {
        "query": "migration",
        "project": "tree-ring",
        "agent_profile": "default",
        "scope": "project",
        "rings": ["heartwood"],
        "event_types": ["decision"],
        "include_sensitive": True,
        "include_superseded": True,
        "limit": 3,
        "explain_ranking": True,
    }


def test_rust_cli_backend_prefers_configured_binary(tmp_path, monkeypatch):
    monkeypatch.setenv("TREE_RING_MEMORY_CLI", "/tmp/tree-ring --flag")
    memory = RustCliTreeRingMemory.__new__(RustCliTreeRingMemory)

    assert memory._cli_prefix() == ["/tmp/tree-ring", "--flag"]
