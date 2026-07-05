import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from tree_ring_memory import TreeRingMemory
import tree_ring_memory.native_backend as native_backend
from tree_ring_memory.native_backend import NativeTreeRingMemory
from tree_ring_memory.models import MemorySource
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


def test_rust_cli_backend_prefers_configured_binary(tmp_path, monkeypatch):
    monkeypatch.setenv("TREE_RING_MEMORY_CLI", "/tmp/tree-ring --flag")
    memory = RustCliTreeRingMemory.__new__(RustCliTreeRingMemory)

    assert memory._cli_prefix() == ["/tmp/tree-ring", "--flag"]
