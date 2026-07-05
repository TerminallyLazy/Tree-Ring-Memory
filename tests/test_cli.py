import importlib.util
import json
import subprocess
from pathlib import Path

import pytest

import tree_ring_memory
from tree_ring_memory import NativeTreeRingMemory, TreeRingMemory
import tree_ring_memory.native_backend as native_backend
from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource


PROJECT_ROOT = Path(__file__).resolve().parents[1]


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


def test_public_package_exports_rust_only_surface():
    assert hasattr(tree_ring_memory, "TreeRingMemory")
    assert hasattr(tree_ring_memory, "NativeTreeRingMemory")
    assert not hasattr(tree_ring_memory, "PythonTreeRingMemory")
    assert not hasattr(tree_ring_memory, "RustCliTreeRingMemory")


@pytest.mark.parametrize(
    "module_name",
    [
        "tree_ring_memory.cli",
        "tree_ring_memory.store",
        "tree_ring_memory.recall",
        "tree_ring_memory.sensitivity",
        "tree_ring_memory.rust_backend",
    ],
)
def test_python_runtime_modules_are_removed(module_name):
    assert importlib.util.find_spec(module_name) is None


def test_default_facade_requires_native_binding_when_missing(tmp_path):
    with pytest.raises(ImportError, match="native bindings are not installed"):
        TreeRingMemory.open(tmp_path / ".tree-ring")


def test_default_facade_preserves_broken_native_import_error(tmp_path, monkeypatch):
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

    fake_module = type("FakeModule", (), {"TreeRingMemoryNative": FakeNativeStore})
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


def test_native_backend_jsonl_audit_consolidate_and_maintain_delegate_to_binding(tmp_path):
    class FakeNativeStore:
        def export_jsonl(self, include_sensitive=False, include_superseded=False):
            return "exported"

        def import_jsonl(self, data, dry_run=False, replace_existing=False):
            return json.dumps(
                {
                    "valid_count": 1,
                    "inserted_count": 0,
                    "replaced_count": 0,
                    "skipped_duplicate_count": 0,
                    "dry_run": dry_run,
                }
            )

        def audit_json(self, audit_type):
            return json.dumps({"audit_type": audit_type, "memory_count": 0, "finding_count": 0})

        def consolidate_json(self, period_type, period_key, project, dry_run, force):
            return json.dumps(
                {
                    "period_type": period_type,
                    "period_key": period_key,
                    "project": project,
                    "dry_run": dry_run,
                    "force": force,
                    "status": "dry_run",
                }
            )

        def maintain_json(
            self,
            project,
            include_superseded,
            apply_expired,
            apply_secret_redactions,
            repair_fts,
        ):
            return json.dumps(
                {
                    "id": "maint_test",
                    "generated_at": "2026-07-05T00:00:00Z",
                    "memory_count": 3,
                    "planned_action_count": 1,
                    "applied_action_count": 1,
                    "dry_run": False,
                    "status": "applied",
                    "actions": [],
                    "fts": {
                        "memory_rows": 3,
                        "fts_rows": 3,
                        "missing_fts_rows": 0,
                        "orphan_fts_rows": 0,
                        "repaired": True,
                    },
                    "args": [
                        project,
                        include_superseded,
                        apply_expired,
                        apply_secret_redactions,
                        repair_fts,
                    ],
                }
            )

    memory = NativeTreeRingMemory(FakeNativeStore(), tmp_path / ".tree-ring")

    assert memory.export_jsonl() == "exported"
    assert memory.import_jsonl("data", dry_run=True)["dry_run"] is True
    assert memory.audit("all")["audit_type"] == "all"
    assert memory.consolidate(period_type="manual", period_key="k", project="p", dry_run=True)["status"] == "dry_run"
    maintenance = memory.maintain(
        project="core",
        include_superseded=True,
        apply_expired=True,
        apply_secret_redactions=True,
        repair_fts=True,
    )
    assert maintenance["status"] == "applied"
    assert maintenance["args"] == ["core", True, True, True, True]


def test_rust_cli_json_contract_for_init_remember_recall_and_forget(tmp_path):
    root = tmp_path / ".tree-ring"

    init = run_rust_cli(root, "--json", "init")
    assert init.returncode == 0, init.stderr
    assert json.loads(init.stdout)["ok"] is True

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
    assert json.loads(forgotten.stdout) == {"ok": True, "memory_id": memory_payload["id"]}
